
# Table of Contents

1.  [Description](#orga6ad9cb)
2.  [Structure description](#orga01be44)
3.  [Usage](#orgc80c139)
4.  [Notes about feed entry dates](#org5219b30)
    1.  [flat categories](#org4539ff0)
    2.  [tree categories](#orgb7bb8a2)
    3.  [whatever the category is](#org6bd6f2c)



<a id="orga6ad9cb"></a>

# Description

GemAtom is an atom feed generator for Gemini writtent in rust.

It supports a sitre structure description aiming at not being too
stupid in its choice of files to include in the feed.


<a id="orga01be44"></a>

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


<a id="orgc80c139"></a>

# Usage

The general syntax of GemAtom is as follows :

    USAGE:
        gematom [FLAGS] [OPTIONS] --base <URL> --category <DIR:TYPE>... --directory <DIR>
    
    FLAGS:
        -C, --clean-title    When using a file or directory name as a title, convert '_' into space.
        -h, --help           Prints help information
            --mtime          Use file modification time, not file creation time
        -q, --quiet          Do not write on stdout under non-error conditions
        -V, --version        Prints version information
    
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


<a id="org5219b30"></a>

# Notes about feed entry dates


<a id="org4539ff0"></a>

## flat categories

The entry date retained is :

-   if the file name begins with a rfc3339 date, keep this date.
-   otherwise use file creation date, except if `--mtime` flag is
    present.


<a id="orgb7bb8a2"></a>

## tree categories

In tree categories, articles are "index" files contained in
a directory dedicated to this article. Thus :

-   if the parent dir of an article  starts with an rfc3339 date, use this date.
-   otherwise use file creation date, except if `--mtime` flag is
    present.


<a id="org6bd6f2c"></a>

## whatever the category is

If the feed entry title has to be infered from the article file
(flat cat) ou dir (tree cat), then if this file or dir starts with
a rrfc3339 date, it is removed.

