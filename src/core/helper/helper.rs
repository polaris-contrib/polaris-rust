use pole_specification::v1::MatchString;

static WILDCARD: &str = "*";

pub fn is_match_all(s: &str) -> bool {
    s == WILDCARD
}

pub fn match_label_value(rule_value: &MatchString, actual_val: Option<&str>) -> bool {
    if actual_val.is_none() {
        return false;
    }
    let actual_val = actual_val.unwrap();
    let match_value = rule_value.value.clone();
    if is_match_all(&match_value) {
        return true;
    }

    match rule_value.r#type() {
        pole_specification::v1::match_string::MatchStringType::Exact => {
            return match_value == actual_val;
        }
        pole_specification::v1::match_string::MatchStringType::NotEquals => {
            return match_value != actual_val;
        }
        pole_specification::v1::match_string::MatchStringType::Regex => {
            return regex::Regex::new(&match_value)
                .unwrap()
                .is_match(actual_val);
        }
        pole_specification::v1::match_string::MatchStringType::In => {
            return match_value.split(',').any(|x| x == actual_val);
        }
        pole_specification::v1::match_string::MatchStringType::NotIn => {
            return !match_value.split(',').any(|x| x == actual_val);
        }
        pole_specification::v1::match_string::MatchStringType::Range => {
            let parts: Vec<&str> = match_value.split(',').collect();
            if parts.len() != 2 {
                return false;
            }
            let min = parts[0].parse::<i64>().unwrap();
            let max = parts[1].parse::<i64>().unwrap();
            let actual_val = actual_val.parse::<i64>().unwrap();
            return actual_val >= min && actual_val <= max;
        }
    }
}
