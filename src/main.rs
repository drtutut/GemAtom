extern crate atom_syndication;
extern crate glob;
extern crate regex;
extern crate url;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

use std::collections::HashMap;
use std::convert::TryInto;
use std::fs;
use std::io::prelude::*;
use std::io::BufReader;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time;

use atom_syndication::{Entry, Feed, FixedDateTime, Generator, Link, Person};
use chrono::prelude::*;
use chrono::NaiveDateTime;
use clap::{App, Arg, Values};
use glob::glob;
use pathdiff::diff_paths;
use regex::Regex;
use url::Url;

const VERSION: &str = "1.1.1";

lazy_static! {
    static ref RFC3339_RE: Regex = Regex::new(r"^\d{4}-\d{2}-\d{2}").unwrap();
}


/// Categories
#[derive(PartialEq, Copy, Clone, Debug)]
enum Category {
    FLAT,
    TREE,
}

impl Category {
    /// Build a category from a string.
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "flat" => Ok(Category::FLAT),
            "tree" => Ok(Category::TREE),
            _ => Err(format!("Invalid category {}", s)),
        }
    }
}

// useful when collecting files
#[derive(Clone)]
struct Pair(String, Category);

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
        cats.insert(v[0].to_string(), Category::from_str(v[1])?);
    }
    Ok(cats)
}

/// Returns the last modification time of a file.
fn mtime(fname: &str) -> time::SystemTime {
    fs::metadata(fname).unwrap().modified().unwrap()
}

/// Returns the last change time of a file.
fn ctime(fname: &str) -> time::SystemTime {
    fs::metadata(fname).unwrap().created().unwrap()
}

/// Collect all articles in a category.
fn collect_articles(name: &str, typ: Category, root: &str) -> Vec<Pair> {
    let mut fulldir = PathBuf::new();
    fulldir.push(root);
    fulldir.push(name);
    let globs = match typ {
        Category::FLAT => vec!["*.gmi", "*.gemini"],
        Category::TREE => vec!["**/*.gmi", "**/*.gemini"],
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
                        articles.push(Pair(String::from(path.as_path().to_str().unwrap()), typ));
                    }
                }
                _ => {}
            }
        }
    }
    let articles = articles
        .iter()
        .cloned()
        .filter(|e| {
            let Pair(f, _) = e;
            is_world_readable(f)
        })
        .collect();
    return articles;
}

/// Extract the first gemini heading in a file. If no such heading is
/// found, return a default string.
///
/// No check is made concerning the existence of the file.
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

/// Get the feed title.
///
/// If there is an index file, try to extract the first heading,
/// otherwise use the directory name.
fn get_feed_title(dir: &str, clean: bool) -> String {
    let d = Path::new(dir);
    let default = d.file_name().unwrap().to_str().unwrap();
    let default = if clean {
        default.replace("_", " ")
    } else {
        default.to_string()
    };
    for index_file in vec!["index.gemini", "index.gmi"] {
        let mut index_path = PathBuf::new();
        index_path.push(dir);
        index_path.push(index_file);
        let index_path = index_path.to_str().unwrap();
        if is_file(index_path) && is_world_readable(index_path) {
            return extract_first_heading(index_path, &default);
        }
    }
    return default.to_string();
}

/// Extract the files in the specified `categories`, starting from
/// `root` directory. Use `time_func` for sorting. Keep `n` files.
fn get_files(
    root: &str,
    categories: &HashMap<String, Category>,
    time_func: fn(&str) -> time::SystemTime,
    n: usize,
) -> Option<Vec<Pair>> {
    let mut files = Vec::new();
    for (cat, typ) in categories {
        files.extend(collect_articles(cat, *typ, root))
    }
    files.sort_by_key(|a| {
        let Pair(f, _) = a;
        time_func(f)
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

/// Get the update time of a file.
///
/// If the file is in a flat category, then, if the name starts with
/// a rfc3339 date, use it, otherwise use the `time_func`.  If the
/// file is in a tree category, then it is an "index" file. If the
/// parent dir name starts with an rfc3339 date, then use it,
/// otherwise une the `time_func` on the file.
fn get_update_time(
    filepath: &str,
    time_func: fn(&str) -> time::SystemTime,
    cat: Category,
) -> FixedDateTime {
    let path = Path::new(filepath);
    let basename = match cat {
        Category::FLAT => path.file_name().unwrap().to_str().unwrap(),
        Category::TREE => path
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
    };
    if RFC3339_RE.is_match(basename) {
        let date = format!("{}{}", &basename[0..10], "T00:00:00 Z");
        return (&date).parse().unwrap();
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

/// Remove the rfc3339 date in front of a file name if present. If the
/// next chars after the date are '-', '_' or a space, skip them.
fn remove_rfc3339_date(filename: &str) -> &str {
    if RFC3339_RE.is_match(filename) {
        let mut char_indices = filename[10..].char_indices();
        let idx = loop {
            if let Some((idx, ch)) = char_indices.next() {
                if !("_-".contains(ch) || ch.is_whitespace()) {
                    break 10 + idx;
                }
            } else {
                break 10;
            }
        };
        &filename[idx..]
    } else {
        filename
    }
}

/// Set the id, title, updated and link attributes of the provided
/// FeedGenerator entry object according the contents of the named
/// Gemini file and the base URL.
fn populate_entry_from_file(
    filepath: &str,
    base_url: &Url,
    time_func: fn(&str) -> time::SystemTime,
    root: &str,
    cat: Category,
    clean: bool,
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
    entry.set_updated(get_update_time(filepath, time_func, cat));
    let default_title = remove_rfc3339_date(match cat {
        Category::FLAT => pfile.file_stem().unwrap().to_str().unwrap(),
        Category::TREE => pfile
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
    });
    let default_title = if clean {
        default_title.replace("_", " ")
    } else {
        default_title.to_string()
    };
    let title = extract_first_heading(filepath, &default_title);
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
    clean: bool,
) {
    let title = match title {
        Some(t) => String::from(t),
        None => get_feed_title(directory, clean),
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
    let mut gen = Generator::default();
    gen.set_value("gematom, an atom feed generator for gemini.");
    gen.set_uri("https://github.com/drtutut/gematom".to_string());
    gen.set_version(VERSION.to_string());
    feed.set_generator(gen);
    feed.set_rights("© Éric Würbel 2021".to_string());
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

    // TODO: this shoud return a vector of tuples (file, category
    // type) because we need this to infer a default title.
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
    for fp in files {
        let Pair(f, cat) = fp;
        let entry = populate_entry_from_file(&f, &base_url, time_func, directory, cat, clean);
        if verbose {
            println!("Adding {} with title {}", &f, entry.title());
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
        .version(VERSION)
        .author("Eric Würbel <eric@vents-sauvages.fr>")
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
            Arg::with_name("clean-title")
                .short("C")
                .long("clean-title")
                .help("When using a file or directory name as a title, convert '_' into space."),
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
                .help("Use file modification time, not file change time"),
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
    let clean_title = matches.is_present("clean-title");
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
        clean_title,
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
        assert_eq!(is_valid_directory(String::from("/etc")), Ok(()));
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
