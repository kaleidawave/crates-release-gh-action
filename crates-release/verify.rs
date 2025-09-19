use semver::Version;
use simple_toml_parser::{RootTOMLValue, TOMLKey, parse_toml};
use std::collections::HashSet;

static CATEGORIES: &str = "accessibility,aerospace,aerospace::drones,aerospace::protocols,aerospace::simulation,aerospace::space-protocols,aerospace::unmanned-aerial-vehicles,algorithms,api-bindings,asynchronous,authentication,caching,command-line-interface,command-line-utilities,compilers,compression,computer-vision,concurrency,config,cryptography,cryptography::cryptocurrencies,data-structures,database,database-implementations,date-and-time,development-tools,development-tools::build-utils,development-tools::cargo-plugins,development-tools::debugging,development-tools::ffi,development-tools::procedural-macro-helpers,development-tools::profiling,development-tools::testing,email,embedded,emulators,encoding,external-ffi-bindings,filesystem,finance,game-development,game-engines,games,graphics,gui,hardware-support,internationalization,localization,mathematics,memory-management,multimedia,multimedia::audio,multimedia::encoding,multimedia::images,multimedia::video,network-programming,no-std,no-std::no-alloc,os,os::android-apis,os::freebsd-apis,os::linux-apis,os::macos-apis,os::unix-apis,os::windows-apis,parser-implementations,parsing,rendering,rendering::data-formats,rendering::engine,rendering::graphics-api,rust-patterns,science,science::bioinformatics,science::bioinformatics::genomics,science::bioinformatics::proteomics,science::bioinformatics::sequence-analysis,science::geo,science::neuroscience,science::robotics,security,simulation,template-engine,text-editors,text-processing,value-formatting,virtualization,visualization,wasm,web-programming,web-programming::http-client,web-programming::http-server,web-programming::websocket";

static RESERVED: &str = "as,break,const,continue,crate,else,enum,extern,false,fn,for,if,impl,in,let,loop,match,mod,move,mut,pub,ref,return,self,Self,static,struct,super,trait,true,type,unsafe,use,where,while,async,await,dyn,abstract,become,box,do,final,macro,override,priv,typeof,unsized,virtual,yield,try,gen";

// TODO should we also require formatting. aka no `package.sdsd`
// TODO want `simple-toml-parser` to have more positional information
// TODO check unique
// TODO return result
pub fn verify(source: &str) {
    let mut required = HashSet::from(["name"]);
    let mut recommended = HashSet::from(["repository", "5-keywords"]);

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
                    "edition" => {}
                    "rust-version" => {}
                    "default-run" => {}
                    "description" => {}
                    "license" => {}
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

                if *idx == 3 {
                    // remove here
                    // eprintln!("warning: recommened 3 characters");
                }

                if *idx > 4 {
                    eprintln!("Cannot have more than 5 categories");
                }
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
                    if let Some(chr) = value.chars().next() && !chr.is_alphanumeric() {
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

                if *idx > 4 {
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
                TOMLKey::Slice("dependencies" | "dev-dependencies" | "build-dependencies"),
                TOMLKey::Slice(dependency_name),
                rest @ ..,
            ] => {
                // TODO check cfg?
                check_dependency(dependency_name, rest, value)
            }
            [
                TOMLKey::Slice("dependencies" | "dev-dependencies" | "build-dependencies"),
                TOMLKey::Slice(dependency_name),
                rest @ ..,
            ] => check_dependency(dependency_name, rest, value),
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

fn check_dependency(name: &str, keys: &[TOMLKey<'_>], value: RootTOMLValue<'_>) {
    match keys {
        [] => {
            // TODO assert version
        }
        [TOMLKey::Slice("path")] => {
            // TODO assert is in the workspace ...
            eprintln!("Warning: local path {value:?}");
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
