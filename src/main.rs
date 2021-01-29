extern crate atom_syndication;
extern crate glob;
extern crate url;
#[macro_use]
extern crate clap;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time;

use atom_syndication::{Entry, Feed, Link, Person};
use clap::{App, Arg, Values};
use url::{Host, ParseError, Position, Url};
use glob::glob;

enum Category {
    FLAT,
    TREE,
}

impl Category {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "flat" => Ok(Category::FLAT),
            "tree" => Ok(Category::TREE),
            e => Err(format!("Invalid category {}", s)),
        }
    }
}

/// Checks if a category option is well formed
fn is_category(val: String) -> Result<(), String> {
    let v: Vec<&str> = val.split(':').collect();
    if v.len() != 2 {
        return Err(String::from(format!("Bad category specification: {}", val)));
    };
    if ["flat", "tree"].contains(&v[1]) {
        return Ok(());
    };
    Err(String::from(format!("Not a valid category: {}", &v[1])))
}

fn is_gemini_url(val: String) -> Result<(), String> {
    let url = match Url::parse(&val) {
        Ok(u) => u,
        Err(e) => return Err(format!("{}", e)),
    };
    if url.scheme() != "gemini" {
        return Err(format!("Bad url scheme : {}", url.scheme()));
    };
    if url.username() != "" || url.password() != None {
        return Err(format!("user authentication not allowed in url {}", val));
    };
    return Ok(());
}

fn is_valid_directory(val: String) -> Result<(), String> {
    match fs::metadata(&val) {
        Ok(meta) => {
            if meta.is_dir() {
                Ok(())
            } else {
                Err(String::from(format!("Invalid directory: {}", &val)))
            }
        }
        Err(e) => Err(String::from(format!("Invalid directory: {}", &val))),
    }
}

fn is_file(val: &str) -> bool {
    match fs::metadata(val) {
        Ok(meta) => meta.is_file(),
        _ => false,
    }
}

fn is_world_readable(val: &str) -> bool {
    match fs::metadata(val) {
        Ok(meta) => (meta.permissions().mode() & 0o4) != 0,
        _ => false,
    }
}

fn parse_categories(values: &mut Values) -> Result<HashMap<String, Category>, String> {
    let mut cats = HashMap::new();
    for value in values {
        let v: Vec<&str> = value.split(':').collect();
        let c = Category::from_str(v[1])?;
        cats.insert(v[0].to_string(), c);
    }
    Ok(cats)
}

fn mtime(fname: &str) -> time::SystemTime {
    fs::metadata(fname).unwrap().modified().unwrap()
}

fn ctime(fname: &str) -> time::SystemTime {
    fs::metadata(fname).unwrap().created().unwrap()
}

fn collect_articles(name: &str, typ: Category, root: &str) -> Vec<String> {
    let fulldir = PathBuf::new();
    fulldir.push(root);
    fulldir.push(name);
    let globs = Vec::new();
    if typ == Category::FLAT {
        globs.extend(["*.gmi", "*.gemini"]);
    } else {
        globs.extend(["**/*.gmi", "**/*.gemini"]);
    }
    let articles = Vec::new();
    let indexes = vec!["index.gmi", "index.gemini"];
    for pat in globs {
        let mut fullpattern = fulldir.clone();
        fullpattern.push(pat);
        for path in glob(fullpattern) {
            if (!(path.file_name() in indexes) && typ == Category::FLAT) ||
                (path.file_name() in indexes && typ == Category::TREE &&
                )
        }
    return articles;
}

fn extract_first_heading(filename: &str, default: &str) -> String {
    let f = fs::File::open(filename).unwrap();
    let mut reader = BufReader::new(f);
    let mut buffer = String::new();
    while let Ok(n) = reader.read_line(&mut buffer) {
        if n == 0 {
            break;
        }
        let mut buf = &buffer[..];
        if buf.starts_with("#") {
            while buf.chars().nth(0).unwrap() == '#' {
                buf = &buf[1..];
            }
            return String::from(buf.trim());
        }
    }
    return String::from(default);
}

fn get_feed_title(dir: &str) -> String {
    let d = Path::new(dir);
    let default = d.file_name().unwrap().to_str().unwrap();
    for index_file in vec!["index.gemini", "index.gmi"] {
        let index_path = PathBuf::new();
        index_path.push(dir);
        index_path.push(index_file);
        let index_path = index_path.to_str().unwrap();
        if is_file(index_path) && is_world_readable(index_path) {
            return extract_first_heading(index_path, default);
        }
    }
    return default.to_string();
}

fn get_files(
    directory: &str,
    categories: &HashMap<String, Category>,
    time_func: fn(&str) -> time::SystemTime,
    n: usize,
) -> Option<Vec<String>> {
    let mut files = Vec::new();
    for (cat, typ) in categories {
        files.extend(collect_articles(cat, *typ, directory))
    }
    files.sort_by_key(|a| time_func(a));
    files.reverse();
    if files.len() == 0 {
        None
    } else {
        Some(Vec::from(&files[0..n]))
    }
}

fn build_feed(
    directory: &str,
    categories: &HashMap<String, Category>,
    time_func: fn(&str) -> time::SystemTime,
    base_url: Url,
    output: &str,
    n: usize,
    title: Option<&str>,
    subtitle: Option<&str>,
    author: Option<&str>,
    email: Option<&str>,
    verbose: bool,
) {
    let title = match title {
        Some(t) => String::from(t),
        None => get_feed_title(directory),
    };
    let feed_url = base_url.join(output).unwrap();
    if verbose {
        println!(
            "Generating feed \"{}\", which should be served from {}",
            title, feed_url
        );
    }
    let feed = Feed::default();
    feed.set_id(base_url.as_str());
    feed.set_title(title);
    feed.set_subtitle(if let Some(s) = subtitle {
        Some(String::from(s))
    } else {
        None
    });
    let person = Person::default();
    if let Some(a) = author {
        person.set_name(a);
    }
    person.set_email(if let Some(e) = email {
        Some(String::from(e))
    } else {
        None
    });
    if person.name != "" || person.email != None {
        let v = vec![person];
        feed.set_authors(v);
    }
    let self_link = Link::default();
    let alt_link = Link::default();
    self_link.set_href(base_url.as_str());
    self_link.set_rel("href");
    alt_link.set_href(base_url.as_str());
    alt_link.set_rel("alternate");
    let v = vec![self_link, alt_link];
    feed.set_links(v);

    let files = get_files(directory, categories, time_func, n);
    if files == None {
        if verbose {
            println!("No world-readable gemini content found! :(");
        }
        return;
    }
    let mut entries = Vec::new();
}

fn main() {
    let matches = App::new("gematom")
        .version("1.0")
        .author("Eric Würbel <eric.wurbel@univ-amu.fr>")
        .about("Generate an atom feed our of a gemini site")
        .arg(
            Arg::with_name("author")
                .short("a")
                .long("author")
                .value_name("NAME")
                .help("Author name")
                .default_value("")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("base")
                .short("b")
                .long("base")
                .value_name("URL")
                .help("Base URL for feed and entries")
                .validator(is_gemini_url)
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("category")
                .short("c")
                .long("category")
                .value_name("DIR:TYPE")
                .help("Category of a subdir. 'flat' ou 'tree'")
                .multiple(true)
                .required(true)
                .validator(is_category)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("directory")
                .short("d")
                .long("directory")
                .value_name("DIR")
                .help("Root directory of the site")
                .required(true)
                .validator(is_valid_directory)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("email")
                .short("e")
                .long("email")
                .value_name("EMAIL")
                .help("author's email address")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("n")
                .short("n")
                .value_name("N")
                .help("Include N most recently created files in feed (default 10)")
                .default_value("10")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("FILE")
                .help("Output file name")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .help("Do not write on stdout under non-error conditions"),
        )
        .arg(
            Arg::with_name("subtitle")
                .short("s")
                .long("subtitle")
                .value_name("STR")
                .help("Feed subtitle")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("title")
                .short("t")
                .long("title")
                .value_name("STR")
                .help("Feed title")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("mtime")
                .long("mtime")
                .help("Use file modification time, not file update time"),
        )
        .get_matches();
    let base = Url::parse(matches.value_of("base").unwrap()).unwrap();
    let categories = match parse_categories(&mut matches.values_of("category").unwrap()) {
        Ok(cts) => cts,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    let directory = matches.value_of("directory").unwrap();
    let n = value_t!(matches, "n", usize).unwrap_or(10);
    let output = matches.value_of("output").unwrap();
    let verbose = !matches.is_present("quiet");
    let time_func = if matches.is_present("mtime") {
        mtime
    } else {
        ctime
    };

    build_feed(
        directory,
        &categories,
        time_func,
        base,
        output,
        n,
        matches.value_of("title"),
        matches.value_of("subtitle"),
        matches.value_of("author"),
        matches.value_of("email"),
        verbose,
    );
}
