#[macro_use]
extern crate serde_derive;
extern crate checksums;
extern crate clap;
extern crate colored;
extern crate glob;
extern crate globwalk;
extern crate rayon;
extern crate serde_yaml;
extern crate subprocess;
extern crate ignore;

use clap::{App, Arg};

pub mod roomservice;
pub mod util;

use roomservice::config;
use roomservice::room::{Hooks, RoomBuilder};
use roomservice::RoomserviceBuilder;

use std::path::Path;
use util::{fail, unwrap_fail};

fn main() {
    use std::time::Instant;
    let start_time = Instant::now();
    let matches = App::new("Roomservice")
        .arg(
            Arg::with_name("project")
                .short("p")
                .long("project")
                .takes_value(true),
        )
        .arg(Arg::with_name("force").long("force").short("f"))
        .arg(
            Arg::with_name("only")
                .long("only")
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("ignore")
                .long("ignore")
                .takes_value(true)
                .multiple(true),
        )
        .arg(Arg::with_name("after").long("after"))
        .arg(Arg::with_name("dry").long("dry").short("d"))
        .arg(Arg::with_name("dump-scope").long("dump-scope"))
        .arg(Arg::with_name("update-hashes").long("update-hashes"))
        // Hooks
        .arg(Arg::with_name("no-after").long("no-after"))
        .get_matches();

    let project = matches.value_of("project").unwrap_or("./");
    let no_after = matches.is_present("no-after");
    let force = matches.is_present("force");
    let after = matches.is_present("after");

    let only: Vec<_> = match matches.values_of("only") {
        Some(only_values) => only_values.collect(),
        None => vec![],
    };

    let ignore: Vec<_> = match matches.values_of("ignore") {
        Some(ignore_values) => ignore_values.collect(),
        None => vec![],
    };

    if only.len() > 0 && ignore.len() > 0 {
        fail("--only & --ignore options provided, only one of these should be provided at a time")
    }

    if after && no_after {
        fail("Both --after & --no-after options provided.")
    }

    let project_path = unwrap_fail(find_config(project), "No config found.");
    let path_buf = std::path::Path::new(&project_path)
        .canonicalize()
        .unwrap()
        .join(".roomservice");

    let cache_dir = path_buf.to_str().unwrap().to_owned().to_string();

    let mut roomservice = RoomserviceBuilder::new(project_path.clone(), cache_dir.clone(), force);

    let cfg = config::read(&project_path);

    // Check only and ignore values provided are valid
    if only.len() > 0 {
        for name in &only {
            if !cfg.rooms.keys().any(|room_name| room_name == name) {
                println!(
                    "Warning: \"{}\" was provided to --only and does not exist in config",
                    name
                )
            }
        }
    }

    if ignore.len() > 0 {
        for name in &ignore {
            if !cfg.rooms.keys().any(|room_name| room_name == name) {
                println!(
                    "Warning: \"{}\" was provided to --ignore and does not exist in config",
                    name
                )
            }
        }
    }

    for (name, room_config) in cfg.rooms {
        let mut should_add = true;

        // @Note Check to see if it's in the only array
        if only.len() > 0 {
            for only_name in &only {
                if only_name.to_string() != name {
                    should_add = false
                }
            }
        }

        // @Note Check to see if it's in the ignore array
        if ignore.len() > 0 {
            for ignore_name in &ignore {
                if ignore_name.to_string() == name {
                    should_add = false
                }
            }
        }

        if should_add {
            roomservice.add_room(RoomBuilder::new(
                name.to_string(),
                room_config.path.to_string(),
                cache_dir.clone(),
                room_config.include,
                Hooks {
                    before: if after { None } else { room_config.before },
                    run_synchronously: if after {
                        None
                    } else {
                        room_config.run_synchronous
                    },
                    run_parallel: if after {
                        None
                    } else {
                        room_config.run_parallel
                    },
                    after: if no_after { None } else { room_config.after },
                    finally: if after { None } else { room_config.finally },
                },
            ))
        }
    }

    let update_hashes_only = matches.is_present("update-hashes");
    let dry = matches.is_present("dry");
    let dump_scope= matches.is_present("dump-scope");

    roomservice.exec(update_hashes_only, dry, dump_scope);

    println!("\nTime taken: {}s", start_time.elapsed().as_secs())
}

fn find_config(base_path: &str) -> Option<String> {
    let path = Path::new(base_path);
    let maybe_config_path = Path::new(&path).join("roomservice.config.yml");

    if maybe_config_path.exists() {
        return Some(path.to_str().unwrap().to_string());
    } else {
        match maybe_config_path.parent() {
            Some(parent) => {
                if Path::new(parent).exists() {
                    let relative_path = if &base_path[..2] == "./" {
                        Path::new("../").join(&base_path[2..])
                    } else {
                        Path::new("../").join(base_path)
                    };

                    find_config(relative_path.to_str().unwrap())
                } else {
                    None
                }
            }
            None => None,
        }
    }
}
