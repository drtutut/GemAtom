extern crate atom_syndication;
extern crate glob;
extern crate regex;
extern crate url;
#[macro_use]
extern crate clap;

use std::collections::HashMap;
use std::convert::TryInto;
use std::fs;
use std::io::prelude::*;
use std::io::BufReader;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time;

use atom_syndication::{Entry, Feed, FixedDateTime, Link, Person};
use chrono::prelude::*;
use chrono::NaiveDateTime;
use clap::{App, Arg, Values};
use glob::glob;
use pathdiff::diff_paths;
use regex::Regex;
use url::Url;

/// Categories
#[derive(PartialEq, Copy, Clone, Debug)]
enum Category {
    FLAT,
    TREE,
}

impl Category {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "flat" => Ok(Category::FLAT),
            "tree" => Ok(Category::TREE),
            _ => Err(format!("Invalid category {}", s)),
        }
    }
}

/// Checks if a category option is well formed.
/// Called by clap.
fn is_category(val: String) -> Result<(), String> {
    let v: Vec<&str> = val.split(':').collect();
    if v.len() != 2 {
        return Err(String::from(format!("Bad category specification: {}", val)));
    };
    if !["flat", "tree"].contains(&v[1]) {
        return Err(String::from(format!("Not a valid category: {}", &v[1])));
    };
    Ok(())
}

/// Check if string is a valid gemini url.
/// Called by clap.
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

/// Check if a pathname is an existing directory
/// Called by clap.
fn is_valid_directory(val: String) -> Result<(), String> {
    match fs::metadata(&val) {
        Ok(meta) => {
            if meta.is_dir() {
                Ok(())
            } else {
                Err(String::from(format!("Invalid directory: {}", &val)))
            }
        }
        Err(_) => Err(String::from(format!("Invalid directory: {}", &val))),
    }
}

/// Returns true if val is a regular file.
fn is_file(val: &str) -> bool {
    match fs::metadata(val) {
        Ok(meta) => meta.is_file(),
        _ => false,
    }
}

/// Returns true if val is a world readable pathname.
fn is_world_readable(val: &str) -> bool {
    match fs::metadata(val) {
        Ok(meta) => (meta.permissions().mode() & 0o4) != 0,
        _ => false,
    }
}

/// Parses the list of categories given as options.
fn parse_categories(values: &mut Values) -> Result<HashMap<String, Category>, String> {
    let mut cats = HashMap::new();
    for value in values {
        let v: Vec<&str> = value.split(':').collect();
        let c = Category::from_str(v[1])?;
        cats.insert(v[0].to_string(), c);
    }
    Ok(cats)
}

/// Returns the lat modification time of a file.
fn mtime(fname: &str) -> time::SystemTime {
    fs::metadata(fname).unwrap().modified().unwrap()
}

/// Returns the creation time of a file.
fn ctime(fname: &str) -> time::SystemTime {
    fs::metadata(fname).unwrap().created().unwrap()
}

/// Collect all articles in a category.
fn collect_articles(name: &str, typ: Category, root: &str) -> Vec<String> {
    let mut fulldir = PathBuf::new();
    fulldir.push(root);
    fulldir.push(name);
    let globs = if typ == Category::FLAT {
        vec!["*.gmi", "*.gemini"]
    } else {
        vec!["**/*.gmi", "**/*.gemini"]
    };
    let mut articles = Vec::new();
    let indexes = vec!["index.gmi", "index.gemini"];
    for pat in globs {
        let mut fullpattern = fulldir.clone();
        fullpattern.push(pat);
        for path in glob(fullpattern.as_path().to_str().unwrap()).unwrap() {
            match path {
                Ok(path) => {
                    let fname = path.file_name().unwrap().to_str().unwrap();
                    if ((!indexes.contains(&fname)) && typ == Category::FLAT)
                        || (indexes.contains(&fname)
                            && typ == Category::TREE
                            && diff_paths(path.as_path(), fulldir.as_path().to_str().unwrap())
                                .unwrap()
                                .as_path()
                                .to_str()
                                .unwrap()
                                .contains('/'))
                    {
                        articles.push(String::from(path.as_path().to_str().unwrap()));
                    }
                }
                _ => {}
            }
        }
    }
    let articles = articles
        .iter()
        .cloned()
        .filter(|e| is_world_readable(e))
        .collect();
    return articles;
}

fn extract_first_heading(filename: &str, default: &str) -> String {
    let f = fs::File::open(filename).unwrap();
    let reader = BufReader::new(f);
    for line in reader.lines() {
        let line = line.unwrap();
        let mut buf = &line[..];
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
        let mut index_path = PathBuf::new();
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
    files.sort_by_key(|a| {
        time_func(a)
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    files.reverse();
    if files.len() == 0 {
        None
    } else {
        Some(Vec::from(&files[0..n]))
    }
}

fn get_update_time(filepath: &str, time_func: fn(&str) -> time::SystemTime) -> FixedDateTime {
    let path = Path::new(filepath);
    let basename = path.file_name().unwrap().to_str().unwrap();
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap();
    if re.is_match(basename) {
        let date = format!("{}{}", &basename[0..10], "T00:00:00 Z");
        println!("=> {:?}", date);
        return FixedDateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M:%S %z").unwrap();
        // return date.parse::<FixedDateTime>().unwrap();
    }
    let updated = time_func(filepath);

    return FixedDateTime::from_utc(
        NaiveDateTime::from_timestamp(
            updated
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .try_into()
                .unwrap(),
            0,
        ),
        FixedOffset::east(0),
    );
}

/// Set the id, title, updated and link attributes of the provided
/// FeedGenerator entry object according the contents of the named
/// Gemini file and the base URL.
fn populate_entry_from_file(
    filepath: &str,
    base_url: &Url,
    time_func: fn(&str) -> time::SystemTime,
    root: &str,
) -> Entry {
    let pfile = Path::new(filepath);
    let proot = Path::new(root);
    let url = if pfile.parent().unwrap() == proot {
        base_url
            .join(pfile.file_name().unwrap().to_str().unwrap())
            .unwrap()
    } else {
        base_url.join(&filepath[root.len()..]).unwrap()
    };
    let mut entry = Entry::default();
    entry.set_id(url.as_str());
    let mut link = Link::default();
    link.set_href(url.as_str());
    link.set_rel("alternate");
    entry.set_links(vec![link]);
    entry.set_updated(get_update_time(filepath, time_func));
    let default_title = pfile.file_stem().unwrap().to_str().unwrap();
    let title = extract_first_heading(filepath, default_title);
    entry.set_title(title);
    entry
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
    let mut feed = Feed::default();
    feed.set_id(base_url.as_str());
    feed.set_title(title);
    feed.set_subtitle(if let Some(s) = subtitle {
        Some(String::from(s))
    } else {
        None
    });
    let mut person = Person::default();
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
    let mut self_link = Link::default();
    let mut alt_link = Link::default();
    self_link.set_href(base_url.as_str());
    self_link.set_rel("self");
    alt_link.set_href(base_url.as_str());
    alt_link.set_rel("alternate");
    let v = vec![self_link, alt_link];
    feed.set_links(v);

    let files = match get_files(directory, categories, time_func, n) {
        None => {
            if verbose {
                println!("No world-readable gemini content found! :(");
            }
            return;
        }
        Some(f) => f,
    };
    let mut entries = Vec::new();
    for f in files {
        let entry = populate_entry_from_file(&f, &base_url, time_func, directory);
        if verbose {
            println!(
                "Adding {} with title {}",
                &f,
                //                Path::new(&f).file_name().unwrap().to_str().unwrap(),
                entry.title()
            );
        }
        entries.push(entry)
    }
    if entries.len() != 0 {
        feed.set_updated(*entries[0].updated());
        feed.set_entries(entries);
    }
    // write the file.
    let mut outpath = PathBuf::new();
    outpath.push(directory);
    outpath.push(output);
    println!("outputting to {:?}", outpath);
    let out = fs::File::create(outpath).unwrap();
    feed.write_to(out).unwrap();
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
                .default_value("atom.xml")
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
    println!(
        "root dir: {:?}, n: {}, output: {:?}, base: {:?}, categories: {:?}",
        directory, n, output, base, categories
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_category() {
        assert_eq!(is_category(String::from("blabla:flat")), Ok(()));
        assert_eq!(is_category(String::from("news:tree")), Ok(()));
        assert_eq!(
            is_category(String::from("vers:zorgl")),
            Err(String::from("Not a valid category: zorgl"))
        );
        assert_eq!(
            is_category(String::from("vers:ici:tree")),
            Err(String::from("Bad category specification: vers:ici:tree"))
        );
    }

    #[test]
    fn test_is_gemini_url() {
        assert_eq!(
            is_gemini_url(String::from("gemini://retry-abort.org")),
            Ok(())
        );
        assert_eq!(
            is_gemini_url(String::from("gemini://retry-abort.org/")),
            Ok(())
        );
        assert_eq!(
            is_gemini_url(String::from("gemini://retry-abort.org/vers/")),
            Ok(())
        );
        assert_eq!(
            is_gemini_url(String::from("gemini://retry-abort.org/vers/test.gmi")),
            Ok(())
        );
        assert_eq!(
            is_gemini_url(String::from("gemini://user@retry-abort.org/vers/test.gmi")),
            Err(String::from("user authentication not allowed in url gemini://user@retry-abort.org/vers/test.gmi"))
        );
        assert_eq!(
            is_gemini_url(String::from("portnawak")),
            Err(String::from("relative URL without a base"))
        );
        assert_eq!(
            is_gemini_url(String::from("http://portnawak.com")),
            Err(String::from("Bad url scheme : http"))
        );
    }

    #[test]
    fn test_is_valid_directory() {
        assert_eq!(is_valid_directory(String::from("/home/wurbel")), Ok(()));
        assert_eq!(
            is_valid_directory(String::from("/home/zorgl")),
            Err(String::from("Invalid directory: /home/zorgl"))
        );
        assert_eq!(
            is_valid_directory(String::from("/dev/core")),
            Err(String::from("Invalid directory: /dev/core"))
        );
    }

    #[test]
    fn test_is_file() {
        assert!(is_file("/etc/hosts"));
        assert!(!is_file("/etc"));
        assert!(!is_file("/dev/loop0"));
    }

    #[test]
    fn test_is_world_readable() {
        assert!(is_world_readable("/etc/hosts"));
        assert!(!is_world_readable("/etc/shadow"));
    }
}
