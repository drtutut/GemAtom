
# Table of Contents

1.  [Description](#orgc02378a)
2.  [Structure description](#org11b5834)
3.  [Usage](#org4fa39a1)
4.  [Notes about feed entry dates](#orgb50b5dd)
    1.  [flat categories](#org7f74f23)
    2.  [tree categories](#org5564fa9)



<a id="orgc02378a"></a>

# Description

GemAtom is an atom feed generator for Gemini writtent in rust.

It supports a sitre structure description aiming at not being too
stupid in its choice of files to include in the feed.


<a id="org11b5834"></a>

# Structure description

A gemini site is defined by

-   a root directory
-   a root url

The site is supposed to be orginized in *categories*.
A category is a subdirectory in the root directory.

A category can be a *flat* category or a *tree* category.

In a flat category all files in the directory of the category will
be considered for being included in the feed, except `index.gmi` or
`index.gemini` files.

In a tree category, each article is a subdirectory of the category
directory.  The files considered for inclusion in the feed are the
`index.gm` or `index.gemini` files included in subdirectories of the
category directory.

**Example**

The following example illustrate the abve concepts.

Consider a flat category  `texts` and a tree category `noise`.
The root directory of the site is `/var/gemini/space`.

The content is supposed to be (bracketed text is a comment)

    /var/gemini/space
    |-- texts                      [flat category directory]
    |   |-- index.gmi              [not in feed]
    |   |-- foo.gmi                [considered for feed]
    |   |-- bar.gmi                [considered for feed]
    |   `-- 2021-01-15-another.gmi [considered for feed]
    `-- noise                      [tree category directory]
        |-- index.gmi              [not in feed]
        |-- spam-and-eggs          [an article is a dir]
        |   |-- index.gmi          [considered for feed]
        |   `-- spam.mp3           [not in feed]
        `-- spanish-inquisition    [an article is a dir]
            |-- index.gmi          [considered for feed]
            `-- nobody-expects.mp3 [not in feed]


<a id="org4fa39a1"></a>

# Usage

The general syntax of GemAtom is as follows :

    USAGE:
        gematom [FLAGS] [OPTIONS] --base <URL> --category <DIR:TYPE>... --directory <DIR>
    
    FLAGS:
        -h, --help       Prints help information
            --mtime      Use file modification time, not file creation time
        -q, --quiet      Do not write on stdout under non-error conditions
        -V, --version    Prints version information
    
    OPTIONS:
        -a, --author <NAME>             Author name [default: ]
        -b, --base <URL>                Base URL for feed and entries
        -c, --category <DIR:TYPE>...    Category of a subdir. 'flat' ou 'tree'
        -d, --directory <DIR>           Root directory of the site
        -e, --email <EMAIL>             author's email address
        -n <N>                          Include N most recently created files in feed (default 10) [default: 10]
        -o, --output <FILE>             Output file name [default: atom.xml]
        -s, --subtitle <STR>            Feed subtitle
        -t, --title <STR>               Feed title


<a id="orgb50b5dd"></a>

# Notes about feed entry dates


<a id="org7f74f23"></a>

## flat categories

The entry date retained is :

-   if the file name begins with a rfc3339 date, keep this date.
-   otherwise use file creation date, except if `--mtime` flag is
    present.


<a id="org5564fa9"></a>

## tree categories

Actually, the file creation date is used, except if `--mtime` flag is
present.

We plan to add the possibility of using a rfc3339 date prefixing
the name of articles' directories.

