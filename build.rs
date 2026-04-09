fn main() {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/polaris_metric_v2.proto"], &["proto"])
        .expect("compile ratelimit proto failed");

    println!("cargo:rerun-if-changed=proto/polaris_metric_v2.proto");
}
