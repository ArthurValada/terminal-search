use std::{fs, io};
use std::fs::{create_dir, File};
use std::io::Write;
use std::option::Option;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{Parser, Subcommand};
use edit::edit_file;
use home::home_dir;
use inquire::Text;
use log::{error, info, LevelFilter, warn};
use regex::Regex;
use selection::get_text;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Function responsible for redirecting [info!], [warn!] and [error!] to the file whose name is
/// specified in the function call.
fn log_init() {
    use systemd_journal_logger::JournalLog;

    JournalLog::new().unwrap().install().unwrap();
    log::set_max_level(LevelFilter::Info);
}

/// Modularization of the function responsible for opening the generated url in the system's default browser.
fn open_browser(engine: &Engine, term: &str) {
    match engine.url(term) {
        Ok(url) => {
            if open::that(url.clone()).is_ok() {
                info!("Browser opened successfully. Url: {}", url);
            } else {
                error!("Error opening browser.");
            }
        }
        Err(_) => error!("Unable to generate URL"),
    }
}


/// Modularization of the function responsible for opening the specified file in the text editor, terminal or system.
fn open_file(path: PathBuf, terminal: bool, snippet: &str) {
    if terminal {
        match edit_file(path) {
            Ok(_) => { info!("Success in opening the file and saving its contents") }
            Err(e) => { error!("Failure!. Error: {}", e) }
        }
    } else {
        match open::that(path) {
            Ok(_) => info!("{} opened successfully", snippet),
            Err(e) => error!("Error opening {}. Error: {}", snippet, e)
        }
    }
}


/// Modularization for printing the search engine in the terminal in yaml format.
fn print_engine_as_yaml(engine: Engine) {
    if let Ok(element_as_string) = serde_yaml::to_string(&engine) {
        println!("{}", element_as_string);
    } else {
        error!("Error when trying to convert engine {} to yaml.", engine.name);
        eprintln!("Unable to convert engine to yaml")
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


    /// Create a new engine according to the values passed by user on interactive mode
    pub fn prompt_from_user() -> Engine {
        let name = Text::new("What is the name of the search engine?").prompt();
        let url_pattern = Text::new("What is the engine URL pattern?").prompt();
        let pattern = Text::new("What pattern are you using?").prompt();
        let regex = Text::new("What regex should be applied to the search term?").prompt();
        let replacement = Text::new("What should the regex be replaced with?").prompt();

        Engine::new(
            name.unwrap().as_str(),
            url_pattern.unwrap().as_str(),
            pattern.unwrap().as_str(),
            regex.unwrap().as_str(),
            replacement.unwrap().as_str(),
        )
    }

    /// Generate the url based on the data already existing in the [Engine] object and based on the term passed
    /// as argument
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

        if !file_path.exists() {
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
        }
    }


    /// Saves the object contents to a .yaml file
    pub fn save(&self) -> Result<(), io::Error> {
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
        if let Some(content) = &mut self.engines {
            content.retain(|element| element.name != name);
            Ok(())
        } else {
            info!("Attempting to remove an element from a null vector");
            Err(io::Error::new(io::ErrorKind::InvalidData, "Attempting to remove an element from a null vector"))
        }
    }


    /// Allows an engine to be removed based on UUID
    pub fn remove_where_uuid(&mut self, uuid: Uuid) -> Result<(), io::Error> {
        if let Some(content) = &mut self.engines {
            content.retain(|element| element.uuid != uuid);
            Ok(())
        } else {
            info!("Attempting to remove an element from a null vector");
            Err(io::Error::new(io::ErrorKind::InvalidData, "Attempting to remove an element from a null vector"))
        }
    }


    /// Generates a list of the names of the configured search engines
    pub fn names(&self) -> Vec<String> {
        match &self.engines {
            Some(content) => content.iter().map(|element| element.name.clone()).collect(),
            None => vec![],
        }
    }


    /// Returns the default search engine
    pub fn default(&self) -> Option<Engine> {
        match &self.default_engine {
            Some(default) => {
                self.engines.as_ref()?.iter().find(|&element| element.name == *default).cloned()
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


/// Class responsible for intermediate the command line with the executable.
/// [Parser], belonging to *Clap*, is used to generate the implementation for the command line.
/// command macros are used to add information to the command line, according to their name.
#[derive(Parser)]
#[command(author = "Arthur Valadares Campideli", version, about = "An application to open a search term from the command line", long_about = "This application was created with the aim of adding a shortcut to the keyboard in order to search the selected text", subcommand_negates_reqs = true)]
#[command(propagate_version = true)]
struct Cli {
    /// Optional argument. If none is specified, the default will be used
    #[arg(long, short, help = "Specifies the search engine to be used")]
    engine: Option<String>,

    /// Commands that can be executed
    #[command(subcommand)]
    commands: Option<Commands>,

    /// The search term to be used, possibly null, in this case the selected text will be used
    #[arg(num_args(0..), help = "Specify the term to be searched for")]
    term: Option<Vec<String>>,
}


/// Enum containing the subcommands that can be executed from the Cli.
#[derive(Subcommand)]
enum Commands {
    /// Lists the configured search engines
    #[clap(about = "List configured search engines")]
    List,

    /// Defines and shows the default search engine configured
    #[clap(about = "Show the default search engine")]
    Default,

    #[clap(about = "Set the default search engine")]
    SetDefault { name: String },

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
        uuid: bool,
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


/// Enum that contains the set of subcommands that can be executed from the command [Commands::Log]
#[derive(Subcommand)]
#[derive(PartialEq)]
enum LogCommands {
    /// Enables log messages
    #[clap(about = "Enable the log file")]
    Enable,

    /// Disables log messages
    #[clap(about = "Disable the log file")]
    Disable,

    /// Deletes log files
    #[clap(about = "Delete the log file")]
    Delete,
}

fn main() {

    log_init();

    if let Some(home_path) = home_dir() {
        let search_dir = home_path.join(".search");

        if !search_dir.exists() && create_dir(search_dir.clone()).is_err() {
            std::process::exit(1);
        }

        let search_config_path = search_dir.join("search_config.yaml");

        let cli = Cli::parse();

        match Configuration::from(search_config_path.clone()) {
            Ok(mut config) => {

                if let Some(command) = cli.commands {
                    match command {
                        Commands::Add { name, url_pattern, pattern, regex, replacement, force, interactive } => {
                            if interactive {
                                let engine = Engine::prompt_from_user();
                                config.push(engine);
                            } else {
                                let name = name.unwrap();
                                if force || !config.names().contains(&name.clone()) {
                                    config.push(Engine::new(
                                        name.as_str(),
                                        url_pattern.unwrap().as_str(),
                                        pattern.unwrap().as_str(),
                                        regex.unwrap().as_str(),
                                        replacement.unwrap().as_str(),
                                    ));
                                } else {
                                    eprintln!("The config file already contains a search engine named {}", name);
                                }
                            }
                        }
                        Commands::Remove { value, uuid } => {
                            if uuid {
                                if let Ok(uuid) = Uuid::from_str(value.as_str()) {
                                    match config.remove_where_uuid(uuid) {
                                        Ok(_) => info!("Successful removal of {} engine", value),
                                        Err(_) => error!("Failed to remove {} from the search engines list", value),
                                    }
                                } else {
                                    error!("Unable to convert {} to a uuid.", value);
                                }
                            } else {
                                match config.remove_where_name(value.as_str()) {
                                    Ok(_) => info!("Successful removal of {} engine", value),
                                    Err(_) => error!("Failed to remove {} from the search engines list", value),
                                }
                            }
                        }
                        Commands::List => {
                            for name in config.names() {
                                println!("- {}", name);
                            }
                        }
                        Commands::Default => {
                            if let Some(default_engine) = config.default() {
                                println!("- {}", default_engine.name)
                            } else {
                                eprintln!("No default engine defined!")
                            }
                        }
                        Commands::SetDefault { name } => {
                            if config.names().contains(&name) {
                                match config.set_default(name.clone()) {
                                    Ok(_) => { info!("Updated default search engine") }
                                    Err(e) => {
                                        error!("Unable to update default search engine. Error: {}", e);
                                        eprintln!("Unable to update default search engine.");
                                    }
                                }
                            } else {
                                eprintln!("Config file does not contains {} search engine.", name);
                            }
                        }
                        Commands::Show { name, all } => {
                            if let Some(engines) = config.engines.clone() {
                                if all {
                                    for engine in engines {
                                        print_engine_as_yaml(engine);
                                    }
                                } else if let Some(value) = name {
                                    match config.where_name(value.clone()) {
                                        Ok(engine) => print_engine_as_yaml(engine),
                                        Err(_) => warn!("There is no engine defined named {}", value),
                                    }
                                }
                            } else {
                                error!("There are no defined engines");
                            }
                        }
                        Commands::Open { terminal } => {
                            open_file(search_config_path.clone(), terminal, "Configuration file");
                        }
                    }

                    if let Err(e) = config.save() {
                        error!("Failed to save file. Error: {}", e);
                    } else {
                        info!("The file has been saved successfully");
                    }
                } else {
                    let engine = cli.engine.map_or_else(|| config.default().unwrap_or_else(|| {
                        error!("There is no defined default search engine.");
                        std::process::exit(1);
                    }), |engine_name| {
                        config.where_name(engine_name).unwrap_or_else(|_| {
                            error!("Engine not found. Using default search engine.");
                            config.default().expect("No search engine specified.")
                        })
                    });

                    if let Some(queries) = cli.term {
                        for query in queries {
                            open_browser(&engine, query.as_str());
                        }
                    } else {
                        open_browser(&engine, get_text().as_str());
                    }
                }
            }
            Err(_) => {
                error!("There was an error loading file settings");
            }
        }
    }
}
