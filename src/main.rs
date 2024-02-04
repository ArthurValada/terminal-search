use std::{fs, io};
use std::env::current_dir;
use std::fs::File;
use std::io::Write;
use std::option::Option;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, Subcommand};
use edit::edit_file;
use home::home_dir;
use inquire::Text;
use log::{error, info, LevelFilter, warn};
use log4rs;
use log4rs::append::file::FileAppender;
use log4rs::Config;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use regex::Regex;
use selection::get_text;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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


fn open_browser(engine: &Engine, term: &str) {
    match engine.url(term) {
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

/// This class was created with the aim of representing a search engine.
/// It makes use of the macros [Serialize], [Deserialize] and [Parser] so that it can be serialized and deserialized
/// by serde \[feature= serde_yaml] and passed as arguments on the command line. This object contains the
/// minimum settings for the system to function properly, regarding the search engine URL.
#[derive(Serialize, Deserialize, Debug, Parser, Clone)]
pub struct Engine {

    uuid: Uuid,

    /// Represent the name of the search engine
    name: String,

    /// Store the search engine url pattern;
    url_pattern: String,

    /// Store the replacement pattern being used in the url
    pattern: String,

    /// The regex that will be searched within the search term and replaced by replacement
    regex: String,
    replacement: String,
}


/// Implementation of the struct [Engine].
impl Engine {

    /// Create a new engine according to the values passed as arguments;
    pub fn new(name: &str, url_pattern: &str, pattern: &str, regex: &str, replacement: &str) -> Engine {
        info!("Creating a new engine.");
        Engine {
            uuid: Uuid::new_v4(),
            name: String::from(name),
            url_pattern: String::from(url_pattern),
            pattern: pattern.to_string(),
            regex: regex.to_string(),
            replacement: String::from(replacement),
        }
    }


    /// Generate the url based on the data already existing in the [Engine] object and based on the term passed
    /// as argument;
    pub fn url(&self, term: &str) -> Result<String, io::Error> {
        info!("Generating a URL.");

        match Regex::new(self.regex.as_str()) {
            Ok(regex) => {
                let treated_string = regex.replace_all(term, &self.replacement).to_string();
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


/// Class created with the objective of storing all the configurations that the program supports.
/// The [Configuration] class has the macros [Serialize] and [Deserialize], so that it can be serialized and
/// deserialized by serde \[feature=serde_yaml], in order to be written to and read from a .yaml file
#[derive(Serialize, Deserialize, Debug)]
struct Configuration {

    /// Stores the configuration file path;
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    file_path: PathBuf,

    /// Stores the name of the default search engine, null by default and subject to change, according to user preferences
    default_engine: Option<String>,

    /// Stores all objects representing search engines - [Engine]
    engines: Option<Vec<Engine>>,
}


/// Implementation of the Configuration struct.
/// About the macro: In order to provide possibly useful features for what the project may become.
/// Some functions, whose scope is very well-defined, are currently not applicable. To this end, in order
/// to indicate to the compiler that there are no problems with the existence of _dead_ code, this directive is used
#[warn(dead_code)]
impl Configuration {

    /// Responsible for creating a new instance of a configuration object based on the values passed as arguments
    pub fn new(file_path: PathBuf, default_engine: Option<String>, engines: Option<Vec<Engine>>) -> Configuration {
        info!("Creating a new settings.");
        Configuration {
            file_path,
            default_engine,
            engines,
        }
    }


    /// Responsible for loading the configuration object from the file path passed as an argument.
    /// If the file does not exist, it is created, if it exists but is empty, a new default configuration object is
    /// created, if the file exists and is not empty, an attempt is made to load its configuration.
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


    /// Saves the object contents to a .yaml file
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


    /// Adds an engine to the list of configured search engines
    pub fn push(&mut self, engine: Engine) {
        self.engines = self.engines.clone().map_or(Some(vec![engine.clone()]), |mut vector| {
            vector.push(engine);
            Some(vector)
        });
    }


    /// Updates the file path
    pub fn update_path(&mut self, new: PathBuf) {
        self.file_path = new;
    }


    /// Removes a search engine based on name
    pub fn remove_where_name(&mut self, name: &str) -> Result<(), io::Error> {
        return if let Some(content) = &mut self.engines {
            content.retain(|element| element.name != name);
            Ok(())
        } else {
            info!("Attempting to remove an element from a null vector");
            Err(io::Error::new(io::ErrorKind::InvalidData, "Attempting to remove an element from a null vector"))
        };
    }

    pub fn remove_where_uuid(&mut self, uuid: Uuid) -> Result<(), io::Error> {
        return if let Some(content) = &mut self.engines {
          content.retain(|element| element.uuid != uuid);
            Ok(())
        }
        else{
            info!("Attempting to remove an element from a null vector");
            Err(io::Error::new(io::ErrorKind::InvalidData, "Attempting to remove an element from a null vector"))
        };
    }


    /// Removes a search engine based on its position on [self.engines]
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


    /// Generates a list of the names of the configured search engines
    pub fn names(&self) -> Vec<String> {
        match &self.engines {
            Some(content) => content.iter().map(|element| element.name.clone()).collect(),
            None => vec![],
        }
    }


    /// Generates a list of patterns configured for each search engine
    pub fn patterns(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.pattern.clone()).collect(),
            None => vec![]
        }
    }


    /// Generates a list of url patterns from all search engines
    pub fn ulr_patterns(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.url_pattern.clone()).collect(),
            None => vec![],
        }
    }


    /// Generates a list with the regex of each search engine
    pub fn regexes(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.regex.clone()).collect(),
            None => vec![],
        }
    }


    /// Generates a list of replacement for each search engine
    pub fn replacements(self) -> Vec<String> {
        match self.engines {
            Some(content) => content.iter().map(|element| element.replacement.clone()).collect(),
            None => vec![],
        }
    }


    /// Returns the default search engine
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


    /// Sets the default search engine based on name
    pub fn set_default(&mut self, name: String) -> Result<(), io::Error> {
        if self.names().contains(&name) {
            self.default_engine = Some(name);
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::InvalidData, "The search engine passed as an argument is not included in the settings"))
        }
    }


    /// Returns the search engine based on the name passed as an argument
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


/// Class responsible for intermediating the command line with the executable.
/// [Parser], belonging to *Clap*, is used to generate the implementation for the command line.
/// command macros are used to add information to the command line, according to their name.
#[derive(Parser)]
#[command(author = "Arthur Valadares Campideli", version, about = "An application to open a search term from the command line", long_about = "This application was created with the aim of adding a shortcut to the keyboard in order to search the selected text")]
#[command(propagate_version = true)]
struct Cli {

    /// The search term to be used, possibly null, in this case the selected text will be used
    #[arg(num_args(0..), help = "Specify the term to be searched for")]
    term: Option<Vec<String>>,

    /// Optional argument. If none is specified, the default will be used
    #[arg(long, short, help = "Specifies the search engine to be used")]
    engine: Option<String>,

    /// Commands that can be executed
    #[command(subcommand)]
    commands: Option<Commands>,
}


/// Enum containing the subcommands that can be executed from the Cli.
#[derive(Subcommand)]
enum Commands {

    /// Lists the configured search engines
    #[clap(about = "List configured search engines")]
    List,

    /// Defines and shows the default search engine configured
    #[clap(about = "If no arguments are given, show the default search engine, if name is given, set the default search engine")]
    Default {name: Option<String>,},

    /// Adds a search engine based on the values requested by [Engine::new]
    #[clap(about = "Add a search engine")]
    Add {
        #[arg(help = "Search engine name")]
        name: Option<String>,

        #[arg(help = "Search engine url pattern")]
        url_pattern: Option<String>,

        #[arg(help = "Pattern that will be replaced by the treated search term")]
        pattern: Option<String>,

        #[arg(help = "Regex that will be applied to the search term")]
        regex: Option<String>,

        #[arg(help = "Value by which the regex will be replaced")]
        replacement: Option<String>,


        #[arg(short, long, help = "Force the addition of a new search engine with a repeated name")]
        force: bool,

        #[arg(short, long, help = "Adds a new search engine interactively")]
        interactive: bool,
    },

    /// Removes a search engine based on name
    #[clap(about = "Remove a search engine based on name or uuid")]
    Remove {
        value: String,

        #[arg(short, long)]
        uuid: bool
    },

    #[clap(about = "Shows a specific search engine or all")]
    Show {
        name: Option<String>,

        #[arg(short, long, required_unless_present = "name")]
        all: bool,
    },

    #[clap(about = "Open the file containing the settings")]
    Open {
        #[arg(short, long, help = "Open the file in the system's default terminal editor")]
        terminal: bool
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
                        Commands::Add { name, url_pattern, pattern, regex, replacement, force , interactive} => {
                            if interactive {
                                let engine_name = Text::new("What is the name of the search engine?").prompt();
                                let engine_url_pattern = Text::new("What is the engine URL pattern?").prompt();
                                let engine_pattern = Text::new("What pattern are you using?").prompt();
                                let engine_regex = Text::new("What regex should be applied to the search term?").prompt();
                                let engine_replacement = Text::new("What should the regex be replaced with?").prompt();

                                let new_engine = Engine::new(
                                    engine_name.unwrap().as_str(),
                                    engine_url_pattern.unwrap().as_str(),
                                    engine_pattern.unwrap().as_str(),
                                    engine_regex.unwrap().as_str(),
                                    engine_replacement.unwrap().as_str(),
                                );

                                config.push(new_engine);
                            }
                            else{
                                if force || ! config.names().contains(&name.clone().unwrap()) {
                                    config.push(Engine::new(
                                        name.unwrap().as_str(),
                                        url_pattern.unwrap().as_str(),
                                        pattern.unwrap().as_str(),
                                        regex.unwrap().as_str(),
                                        replacement.unwrap().as_str(),
                                    ));
                                }
                                else{
                                    eprintln!("The config file already contains a search engine named {}", name.unwrap())
                                }
                            }
                        }
                        Commands::Remove { value, uuid} => {
                            if uuid {
                                if let Ok(uuid) = Uuid::from_str(value.as_str()){
                                    if let Ok(_) = config.remove_where_uuid(uuid) {
                                        info!("Successful removal of {} engine", value);
                                    } else {
                                        error!("Failed to remove {} from the search engines list", value);
                                    }
                                }
                                else{
                                    error!("Não foi possível converter {} para um uuid.", value);
                                }
                            }
                            else{
                                if let Ok(_) = config.remove_where_name(value.as_str()) {
                                    info!("Successful removal of {} engine", value);
                                } else {
                                    error!("Failed to remove {} from the search engines list", value);
                                }
                            }
                        }
                        Commands::List => {
                            for name in config.names() {
                                println!("- {}", name);
                            }
                        }
                        Commands::Default { name } => {

                            if let Some(value) = name {
                                if let Err(e) = config.set_default(value) {
                                    error!("Failed to update default engine: {}", e);
                                } else {
                                    info!("Updated default engine definition");
                                }
                            }
                            else{
                                if let Some(default_engine) = config.default() {
                                    println!("- {}", default_engine.name)
                                } else {
                                    eprintln!("No default engine defined!")
                                }
                            }
                        }
                        Commands::Show { name, all } => {
                            if let Some(ref engines) = config.engines{
                                if all {
                                    for element in engines {
                                        if let Ok(element_as_string) = serde_yaml::to_string(&element){
                                            println!("{}", element_as_string);
                                        }
                                        else{
                                            error!("Error when trying to convert engine {} to yaml.", element.name);
                                            eprintln!("Unable to convert engine to yaml")
                                        }
                                    }
                                }
                                else if let Some(value) = name{
                                    if let Ok(engine) = config.where_name(value.clone()) {
                                        if let Ok(element_as_string) = serde_yaml::to_string(&engine){
                                            println!("{}", element_as_string);
                                        }
                                        else{
                                            error!("Error when trying to convert engine {} to yaml.", engine.name);
                                            eprintln!("Unable to convert engine to yaml")
                                        }
                                    }
                                    else{
                                        warn!("There is no engine defined named {}", value.clone());
                                        eprintln!("There is no engine defined named {}", value);
                                    }
                                }
                            }
                            else{
                                error!("Attempt to iterate over a null vector. There are no defined engines");
                                eprintln!("There is no engines defined")
                            }
                        }
                        Commands::Open { terminal } => {
                            if terminal {
                                match edit_file(home_path.join(".search_config.yaml")) {
                                    Ok(_) => { info!("Success in opening the file and saving its contents") }
                                    Err(e) => { error!("Falha!. Error: {}", e) }
                                }
                            }
                            else{
                                match open::that(home_path.join(".search_config.yaml")){
                                    Ok(_) => info!("Configuration file opened successfully"),
                                    Err(e) => error!("Error opening configuration file. Error: {}", e)
                                }
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


                    if let Some(value) = cli.term {
                        for query in value {
                            open_browser(&engine, query.as_str());
                        }
                    } else {
                        open_browser(&engine, get_text().as_str())
                    };

                }
            }
            Err(_) => {
                error!("There was an error loading file settings");
            }
        }
    }
}
