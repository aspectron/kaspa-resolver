use crate::imports::*;

#[derive(Default, Debug, Clone)]
pub struct Tpl {
    pub map: HashMap<String, String>,
}

impl<K, V> From<&[(K, V)]> for Tpl
where
    K: Display,
    V: Display,
{
    fn from(value: &[(K, V)]) -> Self {
        let map: HashMap<String, String> = value
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Tpl { map }
    }
}

impl Tpl {
    #[allow(dead_code)]
    pub fn insert<K, V>(&mut self, key: K, value: V)
    where
        K: Display,
        V: Display,
    {
        self.map.insert(key.to_string(), value.to_string());
    }

    pub fn render<S>(&self, template: S) -> String
    where
        S: Display,
    {
        let re = regex::Regex::new(r"\$\{\s*([a-zA-Z0-9_]+)\s*\}").unwrap();
        let template = template.to_string();
        let mut result = template.clone();

        for caps in re.captures_iter(template.as_str()) {
            if let Some(var_name) = caps.get(1) {
                let key = var_name.as_str().trim();
                if let Some(value) = self.map.get(key) {
                    let re = regex::Regex::new(&format!(r"\$\{{\s*{}\s*\}}", key)).unwrap();
                    result = re.replace(result.as_str(), value.as_str()).to_string();
                } else {
                    log_error!("Tpl", "Missing variable: {}", key);
                }
            }
        }

        result
    }
}
