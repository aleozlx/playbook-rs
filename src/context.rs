use yaml_rust::{Yaml, YamlLoader, YamlEmitter};
use std::ops::Index;
use rpds::HashTrieMap;
use std::fmt::{Display, Formatter, Result};
use linked_hash_map::LinkedHashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    data: HashTrieMap<String, CtxObj>
}

#[derive(Clone, Debug, PartialEq)]
pub enum CtxObj {
    Str(String),
    Int(i64),
    Real(f64),
    Bool(bool),
    Array(Vec<Context>),
    Context(Context),
    None
}

impl From<Yaml> for Context {
    fn from(src: Yaml) -> Self {
        let mut context_data = HashTrieMap::new();
        if let Yaml::Hash(raw) = src {
            for (k, v) in raw {
                if let Yaml::String(key) = k {
                    match v {
                        Yaml::String(val) => { context_data.insert_mut(key.to_owned(),
                            CtxObj::Str(val.to_owned()));
                        },
                        Yaml::Boolean(val) => { context_data.insert_mut(key.to_owned(),
                            CtxObj::Bool(val.to_owned()));
                        },
                        Yaml::Integer(val) => { context_data.insert_mut(key.to_owned(),
                            CtxObj::Int(val.to_owned()));
                        },
                        Yaml::Real(val) => { context_data.insert_mut(key.to_owned(),
                            CtxObj::Real(val.parse().unwrap()));
                        },
                        Yaml::Null => { context_data.insert_mut(key.to_owned(),
                            CtxObj::None);
                        },
                        Yaml::Hash(_) => { context_data.insert_mut(key.to_owned(),
                            CtxObj::Context(Context::from(v))); 
                        },
                        Yaml::Array(val) => { context_data.insert_mut(key.to_owned(),
                            CtxObj::Array(val.iter().map(|i| {Context::from(i.clone())}).collect())); 
                        },
                        Yaml::Alias(_val) => {
                            unimplemented!();
                        },
                        Yaml::BadValue => { }
                    }
                }
            }
        }
        Context { data: context_data }
    }
}

impl<'a> From<&'a str> for Context {
    fn from(s: &str) -> Self {
        Context::from(YamlLoader::load_from_str(s).unwrap()[0].clone())
    }
}

impl Into<Yaml> for Context {
    fn into(self) -> Yaml {
        let mut map = LinkedHashMap::new();
        for (k, v) in self.data.iter() {
            map.insert(Yaml::String(k.to_owned()), match v {
                CtxObj::Str(val) => Yaml::String(val.to_owned()),
                CtxObj::Bool(val) => Yaml::Boolean(val.to_owned()),
                CtxObj::Int(val) => Yaml::Integer(val.to_owned()),
                CtxObj::Real(val) => Yaml::Real(val.to_string()),
                CtxObj::None => Yaml::Null,
                CtxObj::Context(val) => val.clone().into(),
                CtxObj::Array(val) => Yaml::Array(val.iter().map(|i| {i.clone().into()}).collect())
            });
        }
        Yaml::Hash(map)
    }
}

impl Display for Context {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut out_str = String::new();
        {
            let mut emitter = YamlEmitter::new(&mut out_str);
            emitter.dump(&self.clone().into()).unwrap();
        }
        write!(f, "{}", &out_str)
    }
}

impl<'a> Index<&'a str> for Context {
    type Output = CtxObj;

    fn index(&self, key: &'a str) -> &CtxObj {
        self.data.get(key).expect("no entry found for key")
    }
}

impl Context {
    pub fn overlay(&self, another: Context) -> Context {
        let mut ret = self.data.clone();
        for (k, v) in another.data.iter() {
            ret = ret.insert(k.to_owned(), v.to_owned());
        }
        Context { data: ret }
    }

    pub fn assign(&self, key: &str, val: CtxObj) -> Context {
        Context { data: self.data.clone().insert(key.to_owned(), val) }
    }
}

#[cfg(test)]
mod tests{
    use yaml_rust::YamlLoader;
    use context::Context;

    #[test]
    fn multiple_overwrites() {
        let a = Context::from("a: 1\nb: 0");
        let b = Context::from("a: 0\nb: 1");
        let c = a.overlay(b.clone());
        assert_eq!(c, b);
    }

    #[test]
    fn single_overwrite() {
        let a = Context::from("a: 1\nb: 0");
        let b = Context::from("b: 1");
        let c = a.overlay(b);
        assert_eq!(c, Context::from("a: 1\nb: 1"));
    }

    #[test]
    fn insertion() {
        let a = Context::from("a: 1\nb: 0");
        let b = Context::from("c: 1");
        let c = a.overlay(b);
        assert_eq!(c, Context::from("a: 1\nb: 0\nc: 1"));
    }
}
