// Tencent is pleased to support the open source community by making Polaris available.
//
// Copyright (C) 2019 THL A29 Limited, a Tencent company. All rights reserved.
//
// Licensed under the BSD 3-Clause License (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://opensource.org/licenses/BSD-3-Clause
//
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::core::{
    model::{
        error::PolarisError,
        pb::lib::{
            config_discover_request::ConfigDiscoverRequestType,
            config_discover_response::ConfigDiscoverResponseType, ConfigDiscoverRequest,
        },
        DiscoverRequestInfo, DiscoverResponseInfo,
    },
    plugin::{filter::DiscoverFilter, plugins::Plugin},
};

use aes::Aes128;
use base64::{engine::general_purpose::STANDARD as base64_standard, Engine as _};
use block_modes::{block_padding::Pkcs7, BlockMode, Cbc};
use rsa::{pkcs1::EncodeRsaPublicKey, pkcs8::LineEnding, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct CryptoConfig {
    #[serde(rename = "entries")]
    entries: Vec<ConfigEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigEntry {
    #[serde(rename = "name")]
    name: String,
    options: Option<HashMap<String, String>>,
}

/// Cryptor
trait Cryptor
where
    Self: Send + Sync,
{
    /// encrypt
    fn encrypt(&self, plaintext: String, key: String) -> Result<String, PolarisError>;

    /// decrypt
    fn decrypt(&self, ciphertext: String, key: String) -> Result<String, PolarisError>;
}

fn load_cryptors(conf: CryptoConfig) -> HashMap<String, Box<dyn Cryptor>> {
    let mut cryptors = HashMap::<String, Box<dyn Cryptor>>::new();

    cryptors.insert("AES".to_string(), Box::new(AESCryptor {}));

    let mut repo = HashMap::<String, Box<dyn Cryptor>>::new();
    for (_k, v) in conf.entries.iter().enumerate() {
        let name = v.name.as_str();
        let val_opt = cryptors.remove_entry(name);
        match val_opt {
            Some(val) => {
                repo.insert(name.to_string(), val.1);
            }
            None => {
                tracing::error!(
                    "[polaris][plugin][config_filter] crypto not found expect algo: {}",
                    name
                );
            }
        }
    }
    repo
}

pub fn new_filter(plugin_opt: serde_yaml::Value) -> Result<Box<dyn DiscoverFilter>, PolarisError> {
    let rsa_cryptor = RSACryptor::new();
    if rsa_cryptor.is_err() {
        return Err(rsa_cryptor.err().unwrap());
    }

    let conf: CryptoConfig = serde_yaml::from_value(plugin_opt).unwrap();

    Ok(Box::new(ConfigFileCryptoFilter {
        rsa_cryptor: rsa_cryptor.unwrap(),
        cryptors: Arc::new(RwLock::new(load_cryptors(conf))),
    }) as Box<dyn DiscoverFilter>)
}

pub struct ConfigFileCryptoFilter {
    rsa_cryptor: RSACryptor,
    cryptors: Arc<RwLock<HashMap<String, Box<dyn Cryptor>>>>,
}

impl ConfigFileCryptoFilter {
    pub fn builder() -> (
        fn(serde_yaml::Value) -> Result<Box<dyn DiscoverFilter>, PolarisError>,
        String,
    ) {
        (new_filter, "crypto".to_string())
    }
}

impl Plugin for ConfigFileCryptoFilter {
    fn name(&self) -> String {
        "crypto".to_string()
    }

    fn init(&mut self) {}

    fn destroy(&self) {}
}

impl DiscoverFilter for ConfigFileCryptoFilter {
    fn request_process(
        &self,
        request: crate::core::model::DiscoverRequestInfo,
    ) -> Result<crate::core::model::DiscoverRequestInfo, crate::core::model::error::PolarisError>
    {
        let expect_req = request.to_config_request();
        if expect_req.r#type() != ConfigDiscoverRequestType::ConfigFile {
            return Ok(request);
        }
        let pub_key = self.rsa_cryptor.public_key.clone();
        let encrypt_key = base64::prelude::BASE64_STANDARD.encode(pub_key);
        let mut config_file = expect_req.config_file.unwrap().clone();
        config_file.public_key = Some(encrypt_key);
        Ok(DiscoverRequestInfo::Configuration(ConfigDiscoverRequest {
            r#type: ConfigDiscoverRequestType::ConfigFile.into(),
            config_file: Some(config_file),
            revision: expect_req.revision,
        }))
    }

    fn response_process(
        &self,
        response: crate::core::model::DiscoverResponseInfo,
    ) -> Result<crate::core::model::DiscoverResponseInfo, crate::core::model::error::PolarisError>
    {
        let mut expect_rsp = response.to_config_response();
        if expect_rsp.r#type() != ConfigDiscoverResponseType::ConfigFile {
            return Ok(response);
        }

        let mut config_file = expect_rsp.config_file.unwrap().clone();
        let encrypted = config_file.encrypted.unwrap_or(false);
        if !encrypted {
            return Ok(response);
        }
        // 通过 rsa 将 data_key 解密
        let encrypt_data_key = self
            .rsa_cryptor
            .decrypt_from_base64(config_file.get_encrypt_data_key());
        let data_key: String;
        match encrypt_data_key {
            Ok(key) => {
                data_key = key;
            }
            Err(err) => {
                tracing::error!(
                    "[polaris][plugin][config_filter] cipher datakey use rsa decrypt fail: {}",
                    err
                );
                let u8_slice = base64_standard
                    .decode(config_file.get_encrypt_data_key())
                    .unwrap();
                data_key = String::from_utf8(u8_slice).unwrap();
            }
        }

        let algo = config_file.get_encrypt_algo();

        let repo = self.cryptors.read().unwrap();
        let cryptor_opt = repo.get(&algo);
        match cryptor_opt {
            Some(cryptor) => {
                let source_content = config_file.content;
                let decrypted = cryptor.decrypt(source_content.unwrap(), data_key);
                match decrypted {
                    Ok(decrypted) => {
                        config_file.content = Some(decrypted);
                        expect_rsp.config_file = Some(config_file);
                        Ok(DiscoverResponseInfo::Configuration(expect_rsp))
                    }
                    Err(err) => Err(err),
                }
            }
            None => Err(PolarisError::new(
                crate::core::model::error::ErrorCode::ConfigCryptoError,
                format!("unsupported encrypt algorithm: {}", algo),
            )),
        }
    }
}

struct RSACryptor {
    public_key: String,
    priv_key: RsaPrivateKey,
    _pub_key: RsaPublicKey,
}

impl RSACryptor {
    fn new() -> Result<RSACryptor, PolarisError> {
        let mut rng = rand::thread_rng();
        let bits = 1024;
        let priv_key_ret = RsaPrivateKey::new(&mut rng, bits);
        if priv_key_ret.is_err() {
            return Err(PolarisError::new(
                crate::core::model::error::ErrorCode::RsaKeyGenerateError,
                format!(
                    "failed to generate rsa key: {}",
                    priv_key_ret.err().unwrap()
                ),
            ));
        }
        let priv_key = priv_key_ret.unwrap();
        let pub_key = RsaPublicKey::from(&priv_key);
        let public_key_base64 = pub_key.to_pkcs1_pem(LineEnding::LF);
        if public_key_base64.is_err() {
            return Err(PolarisError::new(
                crate::core::model::error::ErrorCode::RsaKeyGenerateError,
                format!(
                    "failed to generate rsa key: {}",
                    public_key_base64.err().unwrap()
                ),
            ));
        }
        Ok(RSACryptor {
            priv_key,
            _pub_key: pub_key,
            public_key: public_key_base64.unwrap(),
        })
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, PolarisError> {
        let mut rng = rand::thread_rng();
        let ret = self
            ._pub_key
            .encrypt(&mut rng, rsa::Pkcs1v15Encrypt, plaintext);
        if ret.is_err() {
            return Err(PolarisError::new(
                crate::core::model::error::ErrorCode::AesDecryptError,
                format!("failed to encrypt: {}", ret.err().unwrap()),
            ));
        }
        Ok(ret.unwrap())
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, PolarisError> {
        let ret = self.priv_key.decrypt(rsa::Pkcs1v15Encrypt, ciphertext);
        if ret.is_err() {
            return Err(PolarisError::new(
                crate::core::model::error::ErrorCode::AesDecryptError,
                format!("failed to decrypt: {}", ret.err().unwrap()),
            ));
        }
        Ok(ret.unwrap())
    }

    fn encrypt_to_base64(&self, plaintext: &[u8]) -> Result<String, PolarisError> {
        let ciphertext = self.encrypt(plaintext);
        match ciphertext {
            Ok(ciphertext) => {
                let ret =
                    base64_standard.encode(unsafe { String::from_utf8_unchecked(ciphertext) });
                Ok(ret)
            }
            Err(e) => Err(e),
        }
    }

    fn decrypt_from_base64(&self, base64_ciphertext: String) -> Result<String, PolarisError> {
        let ret = base64_standard.decode(base64_ciphertext);
        match ret {
            Ok(ciphertext) => {
                let ret = self.decrypt(ciphertext.as_slice());
                match ret {
                    Ok(txt) => Ok(unsafe { String::from_utf8_unchecked(txt) }),
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(PolarisError::new(
                crate::core::model::error::ErrorCode::AesDecryptError,
                err.to_string(),
            )),
        }
    }
}

type Aes256Cbc = Cbc<Aes128, Pkcs7>;

struct AESCryptor {}

impl Cryptor for AESCryptor {
    fn encrypt(&self, plaintext: String, key: String) -> Result<String, PolarisError> {
        let key_slice = key.as_bytes();
        let iv = key.as_bytes()[0..16].to_vec();
        let cipher = Aes256Cbc::new_from_slices(key_slice, &iv).unwrap();
        let encrypted_data = cipher.encrypt_vec(plaintext.as_bytes());

        Ok(base64_standard.encode(encrypted_data))
    }

    fn decrypt(&self, ciphertext: String, key: String) -> Result<String, PolarisError> {
        let key_slice = key.as_bytes();
        let iv = key.as_bytes()[0..16].to_vec();
        let cipher = Aes256Cbc::new_from_slices(key_slice, &iv).unwrap();
        let ret = base64_standard.decode(ciphertext);
        match ret {
            Ok(txt) => {
                let encrypted_data = cipher.decrypt_vec(txt.as_slice());
                match encrypted_data {
                    Ok(data) => {
                        let ret = String::from_utf8(data);
                        match ret {
                            Ok(txt) => Ok(txt),
                            Err(_) => Err(PolarisError::new(
                                crate::core::model::error::ErrorCode::AesDecryptError,
                                "failed to convert decrypted data to string".to_string(),
                            )),
                        }
                    }
                    Err(err) => Err(PolarisError::new(
                        crate::core::model::error::ErrorCode::AesDecryptError,
                        format!("failed to decrypt: {}", err),
                    )),
                }
            }
            Err(err) => Err(PolarisError::new(
                crate::core::model::error::ErrorCode::AesDecryptError,
                format!("failed to decode base64 ciphertext: {}", err),
            )),
        }
    }
}

mod tests {
    

    #[test]
    fn test_rsa_encrypt_decrypt() {
        let cryptor = RSACryptor::new().unwrap();
        let plaintext = "hello world".as_bytes();
        let ciphertext = cryptor.encrypt_to_base64(plaintext).unwrap();
        let decrypted = cryptor.decrypt_from_base64(ciphertext).unwrap();
        assert_eq!(plaintext, decrypted.as_bytes());
    }

    #[test]
    fn test_aes_encrypt_decrypt() {
        let cryptor = AESCryptor {};
        let plaintext = "hello world".to_string();
        let key = "1234567890123456".to_string();
        let ciphertext = cryptor.encrypt(plaintext.clone(), key.clone()).unwrap();
        let decrypted = cryptor.decrypt(ciphertext, key).unwrap();
        assert_eq!(plaintext, decrypted);
    }
}
