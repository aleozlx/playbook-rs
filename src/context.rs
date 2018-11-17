use yaml_rust::Yaml;
use std::ops::Index;
use rpds::HashTrieMap;

#[derive(Clone)]
pub struct Context {
    data: HashTrieMap<String, CtxObj>
}

#[derive(Clone)]
pub enum CtxObj {
    Str(String),
    Int(i64),
    Real(f64),
    Bool(bool),
    Array(Vec<Context>),
    Context(Context),
    None
}

impl<'a> From<&'a Yaml> for Context {
    fn from(src: &'a Yaml) -> Self {
        let mut context_data = HashTrieMap::new();
        if let Yaml::Hash(raw) = src {
            for (k, v) in raw {
                if let Yaml::String(key) = k {
                    match v {
                        Yaml::String(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Str(val.to_owned()));
                        },
                        Yaml::Boolean(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Bool(val.to_owned()));
                        },
                        Yaml::Integer(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Int(val.to_owned()));
                        },
                        Yaml::Real(val) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Real(val.parse().unwrap()));
                        },
                        Yaml::Null => { context_data.insert_mut(
                            key.to_owned(), CtxObj::None);
                        },
                        Yaml::Hash(_) => { context_data.insert_mut(
                            key.to_owned(), CtxObj::Context(Context::from(v))); 
                        },
                        Yaml::Array(val) => {
                            let vv: Vec<Context> = val.iter().map(|i| {Context::from(i)}).collect();
                            context_data.insert_mut(key.to_owned(), CtxObj::Array(vv)); 
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

impl<'a> Index<&'a str> for Context {
    type Output = CtxObj;

    fn index(&self, key: &'a str) -> &CtxObj {
        self.data.get(key).expect("no entry found for key")
    }
}

impl Context {
    fn overlay(&self, another: &Context) -> Context {
        let mut ret = self.data.clone();
        for (k, v) in another.data.iter() {
            ret = ret.insert(k.to_owned(), v.to_owned());
        }
        Context { data: ret }
    }
}