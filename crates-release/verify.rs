use semver::Version;
use simple_toml_parser::{RootTOMLValue, TOMLKey, parse_toml};
use std::collections::HashSet;
use std::path::Path;

pub static CATEGORIES: &str = "accessibility,aerospace,aerospace::drones,aerospace::protocols,aerospace::simulation,aerospace::space-protocols,aerospace::unmanned-aerial-vehicles,algorithms,api-bindings,asynchronous,authentication,caching,command-line-interface,command-line-utilities,compilers,compression,computer-vision,concurrency,config,cryptography,cryptography::cryptocurrencies,data-structures,database,database-implementations,date-and-time,development-tools,development-tools::build-utils,development-tools::cargo-plugins,development-tools::debugging,development-tools::ffi,development-tools::procedural-macro-helpers,development-tools::profiling,development-tools::testing,email,embedded,emulators,encoding,external-ffi-bindings,filesystem,finance,game-development,game-engines,games,graphics,gui,hardware-support,internationalization,localization,mathematics,memory-management,multimedia,multimedia::audio,multimedia::encoding,multimedia::images,multimedia::video,network-programming,no-std,no-std::no-alloc,os,os::android-apis,os::freebsd-apis,os::linux-apis,os::macos-apis,os::unix-apis,os::windows-apis,parser-implementations,parsing,rendering,rendering::data-formats,rendering::engine,rendering::graphics-api,rust-patterns,science,science::bioinformatics,science::bioinformatics::genomics,science::bioinformatics::proteomics,science::bioinformatics::sequence-analysis,science::geo,science::neuroscience,science::robotics,security,simulation,template-engine,text-editors,text-processing,value-formatting,virtualization,visualization,wasm,web-programming,web-programming::http-client,web-programming::http-server,web-programming::websocket";

pub static RESERVED: &str = "as,break,const,continue,crate,else,enum,extern,false,fn,for,if,impl,in,let,loop,match,mod,move,mut,pub,ref,return,self,Self,static,struct,super,trait,true,type,unsafe,use,where,while,async,await,dyn,abstract,become,box,do,final,macro,override,priv,typeof,unsized,virtual,yield,try,gen";

#[derive(Default)]
pub(crate) struct State {
    pub(crate) license: bool,
    pub(crate) last_dependency_name: String,
    pub(crate) last_dev_dependency_name: String,
    pub(crate) last_build_dependency_name: String,
    pub(crate) last_clippy_lint: String,
    pub(crate) last_keyword: String,
    pub(crate) last_category: String,
    pub(crate) keyword_count: usize,
    pub(crate) category_count: usize,
}

impl State {
    pub(crate) fn get_last_dependency_name(&mut self, mode: &str) -> &mut String {
        match mode {
            "dependencies" => &mut self.last_dependency_name,
            "dev-dependencies" => &mut self.last_dev_dependency_name,
            "build-dependencies" => &mut self.last_build_dependency_name,
            name => unreachable!("{name}"),
        }
    }
}

// TODO want `simple-toml-parser` to have more positional information
// TODO check unique
// TODO return result
pub fn verify(source: &str, path: &Path, root: &Path) {
    let mut required = HashSet::from(["name"]);
    let mut recommended = HashSet::from(["version", "edition", "description", "repository"]);

    let mut state = State::default();

    // TODO as parameter
    let lint = true;

    let result = parse_toml(source, |keys, value| {
        match keys {
            [TOMLKey::Slice("package"), TOMLKey::Slice(key)] => {
                match *key {
                    "name" => {
                        // TODO recommended here
                        if let RootTOMLValue::String(content) = value {
                            let name = content.value();
                            if name.len() > 64 {
                                eprintln!("Name must be at most 64 characters, found {name:?}");
                            }
                            if !name.is_ascii() {
                                eprintln!(
                                    "Name must only contain ASCII characters, found {name:?}"
                                );
                            }
                            if RESERVED.split(',').any(|reserved| name == reserved) {
                                eprintln!("Name cannot be reserved, found {name:?}");
                            }
                        } else {
                            todo!("error here")
                        }
                        required.take(key);
                    }
                    "version" => {
                        if let RootTOMLValue::String(content) = value {
                            let value = content.value();
                            let result = Version::parse(&value);
                            if let Err(err) = result {
                                eprintln!("Invalid version {err:?}");
                            }
                        } else {
                            todo!("error here")
                        }
                        // not required
                        recommended.take(key);
                    }
                    "repository" => {
                        if let RootTOMLValue::String(_content) = value {
                            // TODO check exists
                        } else {
                            todo!("error here")
                        }
                        recommended.take(key);
                    }
                    "edition" => {
                        recommended.take(key);
                    }
                    "rust-version" => {}
                    "default-run" => {}
                    "description" => {
                        recommended.take(key);
                    }
                    "license" => {
                        state.license = true;
                    }
                    "license-file" => {
                        // TODO check path
                        state.license = true;
                    }
                    "homepage" => {}
                    "readme" => {}
                    "documentation" => {}
                    "publish" => {
                        if let RootTOMLValue::Boolean(value) = value {
                            eprintln!("package can publish {value:?}");
                        } else {
                            todo!("error here")
                        }
                    }
                    key => {
                        eprintln!("unknown {key:?}")
                    }
                }
            }
            [
                TOMLKey::Slice("package"),
                TOMLKey::Slice("authors"),
                TOMLKey::Index(_idx),
            ] => {
                // TODO...?
            }
            [
                TOMLKey::Slice("package"),
                TOMLKey::Slice("exclude"),
                TOMLKey::Index(_idx),
            ] => {
                // TODO...? check whether needed
            }
            [
                TOMLKey::Slice("package"),
                TOMLKey::Slice("categories"),
                TOMLKey::Index(idx),
            ] => {
                if let RootTOMLValue::String(content) = value {
                    let value = content.value();
                    let contains = CATEGORIES.split(',').any(|category| value == category);
                    if !contains {
                        eprintln!("Invalid category {content:?}");
                    }
                } else {
                    todo!("error here")
                }

                state.category_count = *idx;
            }
            [
                TOMLKey::Slice("package"),
                TOMLKey::Slice("keywords"),
                TOMLKey::Index(idx),
            ] => {
                if let RootTOMLValue::String(content) = value {
                    let value = content.value();
                    if value.len() > 20 {
                        eprintln!("Keyword must be at most 20 characters");
                    }
                    if let Some(chr) = value.chars().next()
                        && !chr.is_alphanumeric()
                    {
                        eprintln!("First char {chr:?} must be alphanumeric");
                    }
                    if let Some(chr) = value.chars().find(|chr| {
                        !(chr.is_ascii()
                            && (chr.is_alphanumeric() || matches!(chr, '_' | '-' | '+')))
                    }) {
                        eprintln!(
                            "Keyword must only contain ASCII letters, numbers, _, - or +. Found {chr:?}"
                        );
                    }
                } else {
                    todo!("error here")
                }

                if *idx == 4 {
                    recommended.take("5-keywords");
                }

                if *idx == 5 {
                    eprintln!("Cannot have more than 5 keywords");
                }
            }
            [TOMLKey::Slice("lib"), rest @ ..] => check_target("lib", rest, value),
            [TOMLKey::Slice("bin"), TOMLKey::Index(_), rest @ ..] => {
                check_target("bin", rest, value)
            }
            [TOMLKey::Slice("test"), TOMLKey::Index(_), rest @ ..] => {
                check_target("test", rest, value)
            }
            [TOMLKey::Slice("example"), TOMLKey::Index(_), rest @ ..] => {
                check_target("example", rest, value)
            }
            [
                TOMLKey::Slice("target"),
                TOMLKey::Slice(_cfg),
                TOMLKey::Slice(mode @ ("dependencies" | "dev-dependencies" | "build-dependencies")),
                TOMLKey::Slice(dependency_name),
                rest @ ..,
            ] => {
                if let [] | [TOMLKey::Slice("version")] = rest
                    && lint
                {
                    let last = state.get_last_dependency_name(mode);
                    if last.as_str() > dependency_name {
                        eprintln!("{dependency_name} should be defined before {last}")
                    }
                    *last = dependency_name.to_string();
                }
                // TODO check cfg?
                check_dependency(dependency_name, rest, value, path, root)
            }
            [
                TOMLKey::Slice(mode @ ("dependencies" | "dev-dependencies" | "build-dependencies")),
                TOMLKey::Slice(dependency_name),
                rest @ ..,
            ] => {
                if let [] | [TOMLKey::Slice("version")] = rest
                    && lint
                {
                    let last = state.get_last_dependency_name(mode);
                    if last.as_str() > dependency_name {
                        eprintln!("{dependency_name} should be defined before {last}")
                    }
                    *last = dependency_name.to_string();
                }
                check_dependency(dependency_name, rest, value, path, root)
            }
            [
                TOMLKey::Slice("features"),
                TOMLKey::Slice("default"),
                TOMLKey::Index(_idx),
            ] => {
                // TODO check things here
            }
            [
                TOMLKey::Slice("features"),
                TOMLKey::Slice(_name),
                TOMLKey::Index(_idx),
            ] => {
                // TODO check things here
            }
            [TOMLKey::Slice("features"), TOMLKey::Slice(_name)] => {
                assert!(matches!(value, RootTOMLValue::EmptyArray));
            }
            [
                TOMLKey::Slice("workspace"),
                TOMLKey::Slice("lints"),
                _rest @ ..,
            ] => {
                // TODO check exists here
            }
            [TOMLKey::Slice("lints"), _rest @ ..] => {
                // TODO check exists here
            }
            [
                TOMLKey::Slice("workspace"),
                TOMLKey::Slice("members"),
                TOMLKey::Index(_),
            ] => {
                // TODO check exists here
            }
            [TOMLKey::Slice("package"), TOMLKey::Slice("metadata"), ..] => {
                // anything allowed here
            }
            keys => {
                eprintln!("unexpected {keys:?} = {value:?}");
                // let start = if let Some(TOMLKey::Slice(slice)) = keys.first() {
                // 	(slice.as_ptr() as usize).checked_sub(source.as_ptr() as usize).expect("slice not in source")
                // } else {
                // 	eprintln!("Unknown");
                // 	0
                // };
                // // TODO context should hold last `=`
                // let end = if let Some(TOMLKey::Slice(slice)) = keys.first() {
                // 	(slice.as_ptr() as usize).checked_sub(source.as_ptr() as usize).expect("slice not in source")
                // } else {
                // 	eprintln!("Unknown");
                // 	0
                // };
            }
        }
    });

    if let Err(err) = result {
        eprintln!("{err:?}");
    }

    if !state.license {
        eprintln!("no license of license-file");
    }

    for required in required {
        eprintln!("{required} is required");
    }

    for recommended in recommended {
        eprintln!("{recommended} is recommended");
    }
}

fn check_target(kind: &str, keys: &[TOMLKey<'_>], value: RootTOMLValue<'_>) {
    match keys {
        [TOMLKey::Slice("name")] => {}
        [TOMLKey::Slice("path")] => {}
        // TODO bool. also warn if default
        [TOMLKey::Slice("test")] => {}
        [TOMLKey::Slice("doctest")] => {}
        [TOMLKey::Slice("bench")] => {}
        [TOMLKey::Slice("doc")] => {}
        [TOMLKey::Slice("proc-macro")] => {
            // TODO only lib
        }
        [TOMLKey::Slice("harness")] => {}
        // TODO assert is one of
        [TOMLKey::Slice("crate-type"), TOMLKey::Index(_)] => {
            if let RootTOMLValue::String(value) = value {
                let value = value.raw();
                static VALID_CRATE_TYPES: &[&str] = &[
                    "bin",
                    "lib",
                    "rlib",
                    "dylib",
                    "cdylib",
                    "staticlib",
                    "proc-macro",
                ];
                if !VALID_CRATE_TYPES.contains(&value) {
                    eprintln!("Invalid crate-type {value}");
                }
            } else {
                panic!();
            }
        }
        // Assert empty array
        [TOMLKey::Slice("required-features")] => {}
        // TODO assert is feature
        [TOMLKey::Slice("required-features"), TOMLKey::Index(_)] => {}
        keys => {
            eprintln!("unexpected {keys:?} in {kind} section");
        }
    }
}

fn check_dependency(
    name: &str,
    keys: &[TOMLKey<'_>],
    value: RootTOMLValue<'_>,
    path: &Path,
    root: &Path,
) {
    match keys {
        [] => {
            // TODO assert version
        }
        [TOMLKey::Slice("path")] => {
            let RootTOMLValue::String(value) = value else {
                panic!("path not string");
            };

            let path = path.join(&*value.value()).join("Cargo.toml");

            // Assert is in the workspace ...
            if !path.starts_with(root) {
                eprintln!("Warning: local path {value:?}");
            }

            assert!(path.exists());
        }
        [TOMLKey::Slice("git")] => {
            eprintln!("Warning: git dependency {value:?}");
        }
        [TOMLKey::Slice("branch")] => {
            // TODO check exists has git etc
            // eprintln!("Warning: git dependency {value:?}");
        }
        [TOMLKey::Slice("version")] => {}
        [TOMLKey::Slice("package")] => {}
        [TOMLKey::Slice("features"), TOMLKey::Index(_)] => {}
        [TOMLKey::Slice("default-features")] => {}
        [TOMLKey::Slice("optional")] => {
            if let RootTOMLValue::Boolean(value) = value {
                if !value {
                    eprintln!("package {name:?} is optional {value:?}");
                }
            } else {
                todo!("error here")
            }
        }
        keys => {
            eprintln!("unexpected {keys:?} in dependency section");
        }
    }
}
