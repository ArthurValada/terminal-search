use std::{fs, io};
use std::fs::File;
use std::io::Write;
use std::option::Option;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use log::{error, info, LevelFilter, warn};
use log4rs;
use log4rs::append::file::FileAppender;
use log4rs::Config;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use regex::Regex;
use selection::get_text;
use serde::{Deserialize, Serialize};
use home::home_dir;

fn log_init(file_path: PathBuf) {
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} | {l} | {m}{n}")))
        .build(file_path)
        .unwrap();

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    log4rs::init_config(config).unwrap();
}

#[derive(Serialize, Deserialize, Debug, Parser, Clone)]
pub struct Engine {
    name: String,
    url_pattern: String,
    pattern: String,
    regex: String,
    replacement: String,
}

impl Engine {
    pub fn new(name: &str, url_pattern: &str, pattern: &str, regex: &str, replacement: &str) -> Engine {
        info!("Creating a new engine.");
        Engine {
            name: String::from(name),
            url_pattern: String::from(url_pattern),
            pattern: pattern.to_string(),
            regex: regex.to_string(),
            replacement: String::from(replacement),
        }
    }

    pub fn url(self, term: &str) -> Result<String, io::Error> {
        info!("Generating a URL.");

        match Regex::new(self.regex.as_str()) {
            Ok(regex) => {
                let treated_string = regex.replace_all(term, self.replacement).to_string();
                info!("Treated string");
                match Regex::new(&regex::escape(self.pattern.as_str())) {
                    Ok(pattern) => {
                        let url = pattern.replace_all(self.url_pattern.as_str(), treated_string).to_string();
                        info!("Url generated successfully: {}", url);
                        Ok(url)
                    }
                    Err(e) => {
                        error!("Unable to generate replacement pattern. Error: {}", e);
                        Err(io::Error::new(io::ErrorKind::Other, e))
                    }
                }
            }
            Err(e) => {
                error!("Failed to generate replacement pattern. Error: {}", e);
                Err(io::Error::new(io::ErrorKind::Other, e))
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Configuration {
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    file_path: PathBuf,

    default_engine: Option<String>,
    engines: Option<Vec<Engine>>,
}

#[warn(dead_code)]
impl Configuration {
    pub fn new(file_path: PathBuf, default_engine: Option<String>, engines: Option<Vec<Engine>>) -> Configuration {
        info!("Creating a new settings.");
        Configuration {
            file_path,
            default_engine,
            engines,
        }
    }

    pub fn from(file_path: PathBuf) -> Result<Configuration, io::Error> {
        info!("Load settings from {:?}", file_path);

        return if !file_path.exists() {
            info!("The configuration file does not exists");
            info!("Creating the configuration file...");
            match File::create(file_path.clone()) {
                Ok(_) => {
                    info!("Success creating configuration file");
                    Ok(Configuration::new(file_path, None, None))
                }
                Err(e) => {
                    error!("Error creating file. Error: {}", e);
                    Err(e)
                }
            }
        } else if fs::metadata(file_path.clone()).map(|metadata| metadata.len() == 0).unwrap_or(true) {
            info!("The config file is empty");
            Ok(Configuration::new(file_path, None, None))
        } else {
            match File::open(file_path.clone()) {
                Ok(file) => {
                    match serde_yaml::from_reader::<File, Configuration>(file) {
                        Ok(mut config) => {
                            info!("Settings loaded successfully");
                            config.update_path(file_path);
                            Ok(config)
                        }
                        Err(error) => {
                            error!("Failed to deserialize YAML: {}", error);
                            Err(io::Error::new(io::ErrorKind::InvalidData, error))
                        }
                    }
                }
                Err(error) => {
                    error!("Failed to open file: {}", error);
                    Err(error)
                }
            }
        };
    }

    pub fn save(self) -> Result<(), io::Error> {
        info!("Trying to save to file {:?}", self.file_path);
        match File::create(self.file_path.clone()) {
            Ok(mut file) => {
                match serde_yaml::to_writer(&file, &self) {
                    Ok(_) => {
                        match file.flush() {
                            Ok(_) => {
                                info!("Configuration saved successfully");
                                Ok(())
                            }
                            Err(e) => {
                                error!("Error saving file: {}", e);
                                Err(e)
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error writing file. Message: {}", e);
                        Err(io::Error::new(io::ErrorKind::Other, e))
                    }
                }
            }
            Err(e) => {
                error!("Failed to open file: {}", e);
                Err(e)
            }
        }
    }

    pub fn push(&mut self, engine: Engine) {
        self.engines = self.engines.clone().map_or(Some(vec![engine.clone()]), |mut vector| {
            vector.push(engine);
            Some(vector)
        });
    }

    pub fn update_path(&mut self, new: PathBuf) {
        self.file_path = new;
    }

    pub fn remove_where_name(&mut self, name: &str) -> Result<(), io::Error> {
        return if let Some(content) = &mut self.engines {
            content.retain(|element| element.name != name);
            Ok(())
        } else {
            info!("Attempting to remove an element from a null vector");
            Err(io::Error::new(io::ErrorKind::InvalidData, "Attempting to remove an element from a null vector"))
        };
    }


    pub fn remove_at(self, index: usize) -> Result<(), io::Error> {
        match self.engines {
            Some(mut content) => {
                content.remove(index);
                Ok(())
            }
            None => {
                info!("Attempting to remove an element from a null vector");
                Err(io::Error::new(io::ErrorKind::InvalidData, "Attempting to remove an element from a null vector"))
            }
        }
    }

    pub fn names(&self) -> Vec<String> {
        match &self.engines {
            Some(content) => content.iter().map(|element| element.name.clone()).collect(),
            None => vec![],
        }
    }

    pub fn patterns(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.pattern.clone()).collect(),
            None => vec![]
        }
    }

    pub fn ulr_patterns(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.url_pattern.clone()).collect(),
            None => vec![],
        }
    }

    pub fn regexes(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.regex.clone()).collect(),
            None => vec![],
        }
    }

    pub fn replacements(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.replacement.clone()).collect(),
            None => vec![],
        }
    }

    pub fn default(&self) -> Option<Engine> {
        match &self.default_engine {
            Some(default) => {
                if let Some(found_element) = self.engines.as_ref()?.iter().find(|&element| element.name == default.to_string()) {
                    Some(found_element.clone())
                } else {
                    None
                }
            }
            None => None
        }
    }

    pub fn set_default(&mut self, name: String) -> Result<(), io::Error> {
        if self.names().contains(&name) {
            self.default_engine = Some(name);
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "The search engine passed as an argument is not included in the settings"))
        }
    }

    pub fn where_name(&self, name: String) -> Result<Engine, io::Error> {
        if let Some(engines) = &self.engines {
            for engine in engines {
                if engine.name == name {
                    return Ok(engine.clone());
                }
            }
            Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid engine name"))
        } else {
            error!("Attempting to get a search engine from a null configuration file");
            Err(io::Error::new(io::ErrorKind::Other, "Attempting to get a search engine from a null configuration file"))
        }
    }
}


#[derive(Parser)]
#[command(author = "Arthur Valadares Campideli", version, about = "A simple test application in rust", long_about = "This application was created with the aim of adding a shortcut to the keyboard in order to search the selected text")]
#[command(propagate_version = true)]
struct Cli {
    #[arg(help = "Specify the term to be searched for")]
    term: Option<String>,

    #[arg(long, short, help = "Specifies the search engine to be used")]
    engine: Option<String>,

    #[command(subcommand)]
    commands: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[clap(about = "List configured search engines")]
    List,

    #[clap(about = "Show default engine")]
    Default,

    #[clap(about = "Sets the default search engine", )]
    SetDefault {
        name: String,
    },

    #[clap(about = "Add a search engine")]
    Add {
        name: String,
        url_pattern: String,
        pattern: String,
        regex: String,
        replacement: String,

    },

    #[clap(about = "Remove a search engine based on name")]
    Remove {
        name: String,
    },
}

fn main() {

    if let Some(home_path) = home_dir() {
        log_init(home_path.join(".search.log"));

        let cli = Cli::parse();

        match Configuration::from(home_path.join(".search_config.yaml")) {
            Ok(mut config) => {
                if let Some(command) = cli.commands {
                    match command {
                        Commands::Add { name, url_pattern, pattern, regex, replacement } => {
                            config.push(Engine::new(name.as_str(), url_pattern.as_str(), pattern.as_str(), regex.as_str(), replacement.as_str()));
                        }
                        Commands::Remove { name: engine_name } => {
                            if let Ok(_) = config.remove_where_name(engine_name.as_str()) {
                                info!("Successful removal of {} engine", engine_name);
                            } else {
                                error!("Failed to remove {} from the search engines list", engine_name);
                            }
                        }
                        Commands::List => {
                            for name in config.names() {
                                println!("- {}", name);
                            }
                        }
                        Commands::SetDefault { name } => {
                            if let Err(e) = config.set_default(name) {
                                error!("Failed to update default engine: {}", e);
                            } else {
                                info!("Updated default engine definition");
                            }
                        }
                        Commands::Default => {
                            if let Some(default_engine) = config.default() {
                                println!("- {}", default_engine.name)
                            } else {
                                eprintln!("No default engine defined!")
                            }
                        }
                    }

                    if let Err(e) = config.save() {
                        error!("Failed to save file. Error: {}", e);
                    } else {
                        info!("The file has been saved successfully");
                    }
                } else {
                    let engine: Engine = if let Some(engine_name) = cli.engine {
                        match config.where_name(engine_name) {
                            Ok(engine) => {
                                info!("Engine found");
                                engine
                            }
                            Err(_) => config.default().unwrap_or_else(|| {
                                error!("There is no defined default search engine.");
                                std::process::exit(1);
                            }),
                        }
                    } else {
                        config.default().expect("No search engine specified.")
                    };


                    let query = if let Some(value) = cli.term {
                        value
                    } else {
                        get_text()
                    };

                    match engine.url(query.as_str()) {
                        Ok(url) => {
                            if let Ok(..) = open::that(url.clone()) {
                                info!("Browser opened successfully. Url: {}", url);
                            } else {
                                error!("Error opening browser.");
                            }
                        }
                        Err(_) => error!("Unable to generate URL"),
                    }
                }
            }
            Err(_) => {
                error!("There was an error loading file settings");
                std::process::exit(1)
            }
        }
    }
    std::process::exit(1)
}
