extern crate yaml_rust;
use self::yaml_rust::Yaml;

#[derive(Clone)]
struct Context {
    data: Vec<(String, ContextValue)>
}

#[derive(Clone)]
enum ContextValue {
    StringValue(String),
    IntValue(i64),
    RealValue(f64),
    BoolValue(bool),
    Context(Context),
    None
}

impl<'a> From<&'a yaml_rust::Yaml> for Context {
    fn from(src: &'a Yaml) -> Self {
        let mut context_data: Vec<(String, ContextValue)> = Vec::new();
        if let Yaml::Hash(raw) = src {
            for (k, v) in raw {
                if let Yaml::String(key) = k {
                    match v {
                        Yaml::String(val) => { context_data.push(
                            (key.to_owned(), ContextValue::StringValue(val.to_owned())));
                        },
                        Yaml::Boolean(val) => { context_data.push(
                            (key.to_owned(), ContextValue::BoolValue(val.to_owned())));
                        },
                        Yaml::Integer(val) => { context_data.push(
                            (key.to_owned(), ContextValue::IntValue(val.to_owned())));
                        },
                        Yaml::Real(val) => { context_data.push(
                            (key.to_owned(), ContextValue::RealValue(val.parse().unwrap())));
                        },
                        Yaml::Null => { context_data.push(
                            (key.to_owned(), ContextValue::None));
                        },
                        Yaml::Hash(_) => { context_data.push(
                            (key.to_owned(), ContextValue::Context(Context::from(v)))); 
                        },
                        _ => ()
                    }
                }
            }
        }
        Context { data: context_data }
    }
}


