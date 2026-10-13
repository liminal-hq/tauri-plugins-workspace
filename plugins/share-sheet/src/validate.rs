// Checks share requests and file paths before any platform share UI sees them
//
// Everything here is platform-independent so it compiles and is tested on every host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::{Component, Path, PathBuf};

use crate::{
    error::{Error, Result},
    models::{ShareFileOptions, ShareTextOptions},
};

/// The most text one `share_text` call takes. Android passes the text through a Binder
/// transaction, which fails well before a megabyte, so the limit leaves room for the rest.
pub const MAX_TEXT_BYTES: usize = 100 * 1024;

/// The most a title may hold, in characters.
pub const MAX_TITLE_CHARS: usize = 200;

/// A file request whose arguments are well formed. The file itself has not been looked at yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRequest {
    pub path: PathBuf,
    pub mime: String,
    pub title: Option<String>,
}

/// A text request whose arguments are well formed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRequest {
    pub text: String,
    pub title: Option<String>,
}

/// Checks the arguments of a file share without touching the file system.
pub fn file_request(options: &ShareFileOptions) -> Result<FileRequest> {
    let raw = options.path.as_str();
    if raw.trim().is_empty() {
        return Err(Error::invalid_path("the path is empty"));
    }
    if raw.contains('\0') {
        return Err(Error::invalid_path("the path contains a NUL character"));
    }
    if raw.starts_with("file:") || raw.starts_with("content:") {
        return Err(Error::invalid_path(format!(
            "expected a file system path, not a URI: {raw}"
        )));
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(Error::invalid_path(format!(
            "the path must be absolute: {raw}"
        )));
    }

    let mime = normalise_mime(&options.mime).ok_or_else(|| {
        Error::invalid_argument(format!(
            "`{}` is not a MIME type of the form type/subtype",
            options.mime
        ))
    })?;

    Ok(FileRequest {
        path,
        mime,
        title: normalise_title(options.title.as_deref())?,
    })
}

/// Checks the arguments of a text share.
pub fn text_request(options: &ShareTextOptions) -> Result<TextRequest> {
    if options.text.trim().is_empty() {
        return Err(Error::invalid_argument("the text is empty"));
    }
    if options.text.len() > MAX_TEXT_BYTES {
        return Err(Error::invalid_argument(format!(
            "the text is {} bytes; the most one share takes is {MAX_TEXT_BYTES}",
            options.text.len()
        )));
    }
    Ok(TextRequest {
        text: options.text.clone(),
        title: normalise_title(options.title.as_deref())?,
    })
}

/// Trims a title, treating an empty one as none.
pub fn normalise_title(title: Option<&str>) -> Result<Option<String>> {
    let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) else {
        return Ok(None);
    };
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(Error::invalid_argument(format!(
            "the title is longer than {MAX_TITLE_CHARS} characters"
        )));
    }
    Ok(Some(title.to_string()))
}

/// Lower-cases a `type/subtype` MIME type, or returns `None` if it is not one. Parameters such
/// as `; charset=utf-8` are refused, because Android matches share targets on the bare type.
pub fn normalise_mime(mime: &str) -> Option<String> {
    let mime = mime.trim();
    let (kind, subtype) = mime.split_once('/')?;
    let token = |part: &str| {
        !part.is_empty()
            && part.len() <= 127
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "!#$&^_.+-".contains(c))
    };
    // A wildcard subtype such as `image/*`, or `*/*`, is allowed: Android accepts both.
    let valid = match (kind, subtype) {
        ("*", "*") => true,
        (kind, "*") => token(kind),
        (kind, subtype) => token(kind) && token(subtype),
    };
    valid.then(|| mime.to_ascii_lowercase())
}

/// Resolves `.` and `..` without touching the file system, so a path can be checked against
/// allowed directories before it is opened.
pub fn normalise_lexically(path: &Path) -> PathBuf {
    let mut normalised = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // `pop` never removes the root or a prefix, so `/..` stays `/`.
                if !matches!(
                    normalised.components().next_back(),
                    None | Some(Component::RootDir | Component::Prefix(_))
                ) {
                    normalised.pop();
                }
            }
            other => normalised.push(other.as_os_str()),
        }
    }
    normalised
}

/// Whether `path` is one of `roots` or lies beneath one, comparing whole components.
pub fn is_within(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

/// Resolves symbolic links and checks that a regular file exists at `path`.
pub fn resolve_existing_file(path: &Path) -> Result<PathBuf> {
    let canonical = std::fs::canonicalize(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => {
            Error::not_found(format!("no file exists at {}", path.display()))
        }
        _ => Error::invalid_path(format!("cannot open {}: {error}", path.display())),
    })?;
    let metadata = std::fs::metadata(&canonical).map_err(|error| {
        Error::invalid_path(format!("cannot read {}: {error}", canonical.display()))
    })?;
    if !metadata.is_file() {
        return Err(Error::invalid_path(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    Ok(canonical)
}

/// Resolves `path` to an existing regular file inside one of `roots`. The path is checked
/// lexically first, so nothing outside the roots is opened (and its existence never reported),
/// and then again after symbolic links are resolved, so a link cannot point outside.
///
/// A root may itself sit behind a link (on Android `/data/user/0` usually links to `/data/data`),
/// so the lexical check accepts a root as given or as resolved, and the final check compares the
/// resolved file with the resolved roots.
pub fn resolve_file_within(path: &Path, roots: &[PathBuf]) -> Result<PathBuf> {
    let roots: Vec<&PathBuf> = roots
        .iter()
        .filter(|root| !root.as_os_str().is_empty() && root.is_absolute())
        .collect();
    let lexical_roots: Vec<PathBuf> = roots.iter().map(|root| normalise_lexically(root)).collect();
    let canonical_roots: Vec<PathBuf> = roots
        .iter()
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .collect();
    let outside = || {
        Error::invalid_path(format!(
            "{} is outside the app's cache and files directories",
            path.display()
        ))
    };
    let lexical = normalise_lexically(path);
    if !is_within(&lexical, &lexical_roots) && !is_within(&lexical, &canonical_roots) {
        return Err(outside());
    }
    let canonical = resolve_existing_file(path)?;
    if !is_within(&canonical, &canonical_roots) {
        return Err(outside());
    }
    Ok(canonical)
}

/// Drops the `\\?\` prefix `std::fs::canonicalize` adds on Windows, so the Windows Runtime
/// storage APIs get the plain form they document. `\\?\UNC\server\share` becomes
/// `\\server\share`; other verbatim paths are left alone.
pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

/// The file's name, for platforms that need a title and were not given one.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "File".to_string())
}

/// The most characters [`text_title`] keeps from the text.
pub const TEXT_TITLE_CHARS: usize = 60;

/// A title for shared text on platforms that need one and were not given one: the first
/// non-empty line, cut to [`TEXT_TITLE_CHARS`] characters with an ellipsis.
pub fn text_title(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Text");
    if line.chars().count() <= TEXT_TITLE_CHARS {
        return line.to_string();
    }
    let cut: String = line.chars().take(TEXT_TITLE_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use std::fs;

    fn file_options(path: &str, mime: &str) -> ShareFileOptions {
        ShareFileOptions {
            path: path.to_string(),
            mime: mime.to_string(),
            title: None,
        }
    }

    /// An absolute path on the host, so the tests run on Windows as well as Unix.
    fn absolute(path: &str) -> String {
        if cfg!(windows) {
            format!(r"C:\{}", path.replace('/', r"\"))
        } else {
            format!("/{path}")
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("share-sheet-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(dir).unwrap()
    }

    #[test]
    fn a_well_formed_file_request_passes_with_its_mime_type_lower_cased() {
        let request = file_request(&ShareFileOptions {
            title: Some("  My save  ".to_string()),
            ..file_options(&absolute("data/save.zip"), "Application/ZIP")
        })
        .unwrap();
        assert_eq!(request.mime, "application/zip");
        assert_eq!(request.title.as_deref(), Some("My save"));
        assert_eq!(request.path, PathBuf::from(absolute("data/save.zip")));
    }

    #[test]
    fn empty_relative_and_uri_paths_are_invalid() {
        for path in [
            "",
            "   ",
            "save.zip",
            "./save.zip",
            "file:///tmp/save.zip",
            "content://x/y",
        ] {
            let error = file_request(&file_options(path, "application/zip")).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidPath, "{path:?}");
        }
    }

    #[test]
    fn a_path_with_a_nul_is_invalid() {
        let error =
            file_request(&file_options(&absolute("a\0b.zip"), "application/zip")).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidPath);
    }

    #[test]
    fn malformed_mime_types_are_invalid_arguments() {
        for mime in [
            "",
            "zip",
            "application/",
            "/zip",
            "application/zip; charset=utf-8",
            "app lication/zip",
            "a/b/c",
            "*/*x",
        ] {
            let error = file_request(&file_options(&absolute("a.zip"), mime)).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidArgument, "{mime:?}");
        }
    }

    #[test]
    fn common_and_wildcard_mime_types_pass() {
        for (mime, expected) in [
            ("application/zip", "application/zip"),
            (
                "application/vnd.lieutenant-fizz.save+zip",
                "application/vnd.lieutenant-fizz.save+zip",
            ),
            (" text/plain ", "text/plain"),
            ("image/*", "image/*"),
            ("*/*", "*/*"),
        ] {
            assert_eq!(normalise_mime(mime).as_deref(), Some(expected), "{mime:?}");
        }
    }

    #[test]
    fn titles_are_trimmed_and_an_empty_one_is_none() {
        assert_eq!(normalise_title(None).unwrap(), None);
        assert_eq!(normalise_title(Some("   ")).unwrap(), None);
        assert_eq!(
            normalise_title(Some(" Fizz ")).unwrap().as_deref(),
            Some("Fizz")
        );
        let long = "x".repeat(MAX_TITLE_CHARS + 1);
        assert_eq!(
            normalise_title(Some(&long)).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        let longest = "é".repeat(MAX_TITLE_CHARS);
        assert!(normalise_title(Some(&longest)).is_ok());
    }

    #[test]
    fn text_must_be_present_and_not_too_long() {
        let text = |text: &str| ShareTextOptions {
            text: text.to_string(),
            title: Some("Title".to_string()),
        };
        assert_eq!(
            text_request(&text("")).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            text_request(&text(" \n ")).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            text_request(&text(&"a".repeat(MAX_TEXT_BYTES + 1)))
                .unwrap_err()
                .code,
            ErrorCode::InvalidArgument
        );
        let ok = text_request(&text(" Beat level 3! ")).unwrap();
        assert_eq!(
            ok.text, " Beat level 3! ",
            "text is passed through untrimmed"
        );
        assert_eq!(ok.title.as_deref(), Some("Title"));
        assert!(text_request(&text(&"a".repeat(MAX_TEXT_BYTES))).is_ok());
    }

    #[test]
    fn lexical_normalisation_resolves_dots_without_escaping_the_root() {
        let n = |p: &str| normalise_lexically(Path::new(p));
        assert_eq!(n("/a/./b/../c"), PathBuf::from("/a/c"));
        assert_eq!(n("/a/b/../../.."), PathBuf::from("/"));
        assert_eq!(n("/../etc/passwd"), PathBuf::from("/etc/passwd"));
        assert_eq!(n("/a//b/"), PathBuf::from("/a/b"));
    }

    #[test]
    fn containment_compares_whole_components() {
        let roots = vec![
            PathBuf::from("/data/user/0/app/cache"),
            PathBuf::from("/data/user/0/app/files"),
        ];
        assert!(is_within(
            Path::new("/data/user/0/app/cache/save.zip"),
            &roots
        ));
        assert!(is_within(
            Path::new("/data/user/0/app/files/a/b.zip"),
            &roots
        ));
        assert!(is_within(Path::new("/data/user/0/app/cache"), &roots));
        assert!(!is_within(
            Path::new("/data/user/0/app/cache-other/save.zip"),
            &roots
        ));
        assert!(!is_within(
            Path::new("/data/user/0/app/databases/app.db"),
            &roots
        ));
        assert!(!is_within(Path::new("/sdcard/save.zip"), &roots));
        assert!(!is_within(Path::new("/data/user/0/app/cache/x"), &[]));
    }

    #[test]
    fn a_file_inside_a_root_resolves_to_its_canonical_path() {
        let dir = scratch("inside");
        let cache = dir.join("cache");
        fs::create_dir_all(cache.join("saves")).unwrap();
        let save = cache.join("saves/save.zip");
        fs::write(&save, b"zip").unwrap();

        let resolved = resolve_file_within(
            &cache.join("saves/../saves/./save.zip"),
            std::slice::from_ref(&cache),
        )
        .unwrap();
        assert_eq!(resolved, save);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dot_dot_out_of_a_root_is_refused_before_the_file_is_looked_at() {
        let dir = scratch("dotdot");
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        fs::write(dir.join("secret.db"), b"secret").unwrap();

        let error = resolve_file_within(&cache.join("../secret.db"), std::slice::from_ref(&cache))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidPath);

        // A missing file outside the roots is still reported as outside, not as missing.
        let error = resolve_file_within(&dir.join("missing.zip"), &[cache]).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidPath);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_file_inside_a_root_is_not_found() {
        let dir = scratch("missing");
        let error =
            resolve_file_within(&dir.join("gone.zip"), std::slice::from_ref(&dir)).unwrap_err();
        assert_eq!(error.code, ErrorCode::NotFound);
        let error = resolve_existing_file(&dir.join("gone.zip")).unwrap_err();
        assert_eq!(error.code, ErrorCode::NotFound);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_directory_is_not_a_file() {
        let dir = scratch("directory");
        fs::create_dir_all(dir.join("saves")).unwrap();
        let error = resolve_existing_file(&dir.join("saves")).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidPath);
        let error =
            resolve_file_within(&dir.join("saves"), std::slice::from_ref(&dir)).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidPath);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_out_of_a_root_is_refused() {
        let dir = scratch("symlink");
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        let secret = dir.join("secret.db");
        fs::write(&secret, b"secret").unwrap();
        std::os::unix::fs::symlink(&secret, cache.join("innocent.zip")).unwrap();

        let error = resolve_file_within(&cache.join("innocent.zip"), &[cache]).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidPath);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_root_behind_a_symbolic_link_accepts_files_through_either_path() {
        let dir = scratch("linkedroot");
        let real = dir.join("data/app/cache");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("save.zip"), b"zip").unwrap();
        std::os::unix::fs::symlink(dir.join("data"), dir.join("user0")).unwrap();
        let linked_root = dir.join("user0/app/cache");

        let through_link = resolve_file_within(
            &linked_root.join("save.zip"),
            std::slice::from_ref(&linked_root),
        )
        .unwrap();
        assert_eq!(through_link, real.join("save.zip"));
        let direct = resolve_file_within(&real.join("save.zip"), &[linked_root]).unwrap();
        assert_eq!(direct, real.join("save.zip"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_roots_allow_nothing() {
        let dir = scratch("emptyroots");
        let file = dir.join("save.zip");
        fs::write(&file, b"zip").unwrap();
        assert_eq!(
            resolve_file_within(&file, &[PathBuf::new()])
                .unwrap_err()
                .code,
            ErrorCode::InvalidPath
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn verbatim_prefixes_are_stripped_for_drive_and_unc_paths() {
        let s = |p: &str| strip_verbatim_prefix(Path::new(p));
        assert_eq!(
            s(r"\\?\C:\Users\fizz\save.zip"),
            PathBuf::from(r"C:\Users\fizz\save.zip")
        );
        assert_eq!(
            s(r"\\?\UNC\server\share\save.zip"),
            PathBuf::from(r"\\server\share\save.zip")
        );
        assert_eq!(
            s(r"\\?\Volume{1234}\save.zip"),
            PathBuf::from(r"\\?\Volume{1234}\save.zip")
        );
        assert_eq!(s(r"C:\save.zip"), PathBuf::from(r"C:\save.zip"));
        assert_eq!(s("/tmp/save.zip"), PathBuf::from("/tmp/save.zip"));
    }

    #[test]
    fn a_text_title_is_the_first_line_cut_short() {
        assert_eq!(
            text_title("\n  Beat level 3!  \nsecond line"),
            "Beat level 3!"
        );
        assert_eq!(text_title("   \n "), "Text");
        let long = "é".repeat(TEXT_TITLE_CHARS + 10);
        let title = text_title(&long);
        assert_eq!(title.chars().count(), TEXT_TITLE_CHARS);
        assert!(title.ends_with('…'));
        let exact = "x".repeat(TEXT_TITLE_CHARS);
        assert_eq!(text_title(&exact), exact);
    }

    #[test]
    fn the_display_name_is_the_file_name() {
        assert_eq!(display_name(Path::new("/a/b/save.zip")), "save.zip");
        assert_eq!(display_name(Path::new("/")), "File");
    }
}
