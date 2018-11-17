use std::collections::BTreeMap;
use yaml_rust::Yaml;
use std::ops::Index;

#[derive(Clone)]
pub struct Context {
    data: BTreeMap<String, ContextValue>
}

#[derive(Clone)]
pub enum ContextValue {
    StringValue(String),
    IntValue(i64),
    RealValue(f64),
    BoolValue(bool),
    Context(Context),
    None
}

impl<'a> From<&'a Yaml> for Context {
    fn from(src: &'a Yaml) -> Self {
        let mut context_data = BTreeMap::new();
        if let Yaml::Hash(raw) = src {
            for (k, v) in raw {
                if let Yaml::String(key) = k {
                    match v {
                        Yaml::String(val) => { context_data.insert(
                            key.to_owned(), ContextValue::StringValue(val.to_owned()));
                        },
                        Yaml::Boolean(val) => { context_data.insert(
                            key.to_owned(), ContextValue::BoolValue(val.to_owned()));
                        },
                        Yaml::Integer(val) => { context_data.insert(
                            key.to_owned(), ContextValue::IntValue(val.to_owned()));
                        },
                        Yaml::Real(val) => { context_data.insert(
                            key.to_owned(), ContextValue::RealValue(val.parse().unwrap()));
                        },
                        Yaml::Null => { context_data.insert(
                            key.to_owned(), ContextValue::None);
                        },
                        Yaml::Hash(_) => { context_data.insert(
                            key.to_owned(), ContextValue::Context(Context::from(v))); 
                        },
                        _ => ()
                    }
                }
            }
        }
        Context { data: context_data }
    }
}

impl<'a> Extend<(&'a String, &'a ContextValue)> for Context {
    fn extend<I: IntoIterator<Item = (&'a String, &'a ContextValue)>>(&mut self, iter: I) {
        self.data.extend(iter.into_iter().map(|(ref key, ref value)| (key, value)));
    }
}

impl<'a> Index<&'a str> for Context {
    type Output = ContextValue;

    fn index(&self, key: &'a str) -> &ContextValue {
        self.data.get(key).expect("no entry found for key")
    }
}
