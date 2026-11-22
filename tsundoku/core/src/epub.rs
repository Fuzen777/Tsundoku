//! Writing an EPUB 3 book (a Pixiv novel's, `pixiv_novel`): a zip holding `mimetype` first and stored, as readers
//! require, `META-INF/container.xml`, the package (`OEBPS/content.opf`), the navigation (`nav.xhtml`, and `toc.ncx`
//! for EPUB 2 readers), a cover page when there's a cover, and one XHTML page per page of the book. Each page's body
//! is given as XHTML already; the rest is written here, every text escaped.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// A book to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    /// What tells it apart for a reader: a URN of its own, such as the novel's page.
    pub identifier: String,
    pub title: String,
    pub author: Option<String>,
    /// Its language, as BCP 47 ("ja").
    pub language: String,
    /// When it was published, as YYYY-MM-DD.
    pub date: Option<String>,
    pub description: Option<String>,
    pub subjects: Vec<String>,
    pub cover: Option<Cover>,
    pub pages: Vec<Page>,
}

/// The book's cover picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub bytes: Vec<u8>,
    /// "image/jpeg" or "image/png".
    pub media_type: String,
}

/// One page of the book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Its chapter's title, for the table of contents, when it starts one.
    pub title: Option<String>,
    /// Its body, as XHTML.
    pub body: String,
}

/// Escapes text for XML.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            // Characters XML 1.0 doesn't allow.
            character if (character as u32) < 0x20 && !matches!(character, '\t' | '\n' | '\r') => {}
            character => escaped.push(character),
        }
    }
    escaped
}

/// The name of the book's `index`th page, from 1.
fn page_name(index: usize) -> String {
    format!("page-{index:04}.xhtml")
}

/// Writes the book to `path`, written beside it first, then moved into place.
pub fn write(book: &Book, path: &Path) -> std::io::Result<()> {
    let written = path.with_extension("epub.part");
    let result = write_to(book, &written).and_then(|()| fs::rename(&written, path));
    if result.is_err() {
        let _ = fs::remove_file(&written);
    }
    result
}

fn write_to(book: &Book, path: &Path) -> std::io::Result<()> {
    let mut zip = ZipWriter::new(File::create(path)?);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let compressed = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let entry =
        |zip: &mut ZipWriter<File>, name: &str, bytes: &[u8], options| -> std::io::Result<()> {
            zip.start_file(name, options)
                .map_err(std::io::Error::other)?;
            zip.write_all(bytes)
        };
    entry(&mut zip, "mimetype", b"application/epub+zip", stored)?;
    entry(
        &mut zip,
        "META-INF/container.xml",
        CONTAINER.as_bytes(),
        compressed,
    )?;
    entry(
        &mut zip,
        "OEBPS/content.opf",
        package(book).as_bytes(),
        compressed,
    )?;
    entry(
        &mut zip,
        "OEBPS/nav.xhtml",
        navigation(book).as_bytes(),
        compressed,
    )?;
    entry(&mut zip, "OEBPS/toc.ncx", ncx(book).as_bytes(), compressed)?;
    entry(&mut zip, "OEBPS/style.css", STYLE.as_bytes(), compressed)?;
    if let Some(cover) = &book.cover {
        entry(
            &mut zip,
            &format!("OEBPS/{}", cover_file(cover)),
            &cover.bytes,
            stored,
        )?;
        entry(
            &mut zip,
            "OEBPS/cover.xhtml",
            cover_page(book, cover).as_bytes(),
            compressed,
        )?;
    }
    for (index, page) in book.pages.iter().enumerate() {
        let name = format!("OEBPS/{}", page_name(index + 1));
        entry(
            &mut zip,
            &name,
            xhtml(book, page.title.as_deref(), &page.body).as_bytes(),
            compressed,
        )?;
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(())
}

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

const STYLE: &str = "body { line-height: 1.8; }\np { margin: 0; text-indent: 1em; }\np.blank { text-indent: 0; }\n\
    h2 { margin: 1em 0; }\nimg.cover { max-width: 100%; max-height: 100%; }\n";

fn cover_file(cover: &Cover) -> &'static str {
    if cover.media_type == "image/png" {
        "cover.png"
    } else {
        "cover.jpg"
    }
}

fn xhtml(book: &Book, title: Option<&str>, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" \
         xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{language}\" lang=\"{language}\">\n<head>\n<meta \
         charset=\"UTF-8\"/>\n<title>{title}</title>\n<link rel=\"stylesheet\" type=\"text/css\" \
         href=\"style.css\"/>\n</head>\n<body>\n{body}\n</body>\n</html>\n",
        language = escape(&book.language),
        title = escape(title.unwrap_or(&book.title)),
    )
}

fn cover_page(book: &Book, cover: &Cover) -> String {
    xhtml(
        book,
        None,
        &format!(
            "<div><img class=\"cover\" src=\"{}\" alt=\"{}\"/></div>",
            cover_file(cover),
            escape(&book.title)
        ),
    )
}

fn package(book: &Book) -> String {
    let mut metadata = format!(
        "<dc:identifier id=\"book-id\">{}</dc:identifier>\n<dc:title>{}</dc:title>\n<dc:language>{}</dc:language>\n",
        escape(&book.identifier),
        escape(&book.title),
        escape(&book.language)
    );
    if let Some(author) = &book.author {
        metadata.push_str(&format!("<dc:creator>{}</dc:creator>\n", escape(author)));
    }
    if let Some(date) = &book.date {
        metadata.push_str(&format!("<dc:date>{}</dc:date>\n", escape(date)));
    }
    if let Some(description) = &book.description {
        metadata.push_str(&format!(
            "<dc:description>{}</dc:description>\n",
            escape(description)
        ));
    }
    for subject in &book.subjects {
        metadata.push_str(&format!("<dc:subject>{}</dc:subject>\n", escape(subject)));
    }
    let modified = book
        .date
        .as_deref()
        .map_or_else(|| "2000-01-01".to_owned(), str::to_owned);
    metadata.push_str(&format!(
        "<meta property=\"dcterms:modified\">{}T00:00:00Z</meta>\n",
        escape(&modified)
    ));
    let mut manifest = String::from(
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n<item \
         id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>\n<item id=\"style\" href=\"style.css\" \
         media-type=\"text/css\"/>\n",
    );
    let mut spine = String::new();
    if let Some(cover) = &book.cover {
        metadata.push_str("<meta name=\"cover\" content=\"cover-image\"/>\n");
        manifest.push_str(&format!(
            "<item id=\"cover-image\" href=\"{}\" media-type=\"{}\" properties=\"cover-image\"/>\n<item \
             id=\"cover\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\"/>\n",
            cover_file(cover),
            escape(&cover.media_type)
        ));
        spine.push_str("<itemref idref=\"cover\"/>\n");
    }
    for index in 1..=book.pages.len() {
        manifest.push_str(&format!(
            "<item id=\"page-{index}\" href=\"{}\" media-type=\"application/xhtml+xml\"/>\n",
            page_name(index)
        ));
        spine.push_str(&format!("<itemref idref=\"page-{index}\"/>\n"));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" \
         unique-identifier=\"book-id\" xml:lang=\"{}\">\n<metadata \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n{metadata}</metadata>\n<manifest>\n{manifest}</manifest>\n<spine \
         toc=\"ncx\">\n{spine}</spine>\n</package>\n",
        escape(&book.language)
    )
}

/// Each entry of the table of contents: its title and page, the book's title for a book without chapters.
fn contents(book: &Book) -> Vec<(String, String)> {
    let chapters: Vec<(String, String)> = book
        .pages
        .iter()
        .enumerate()
        .filter_map(|(index, page)| Some((page.title.clone()?, page_name(index + 1))))
        .collect();
    if chapters.is_empty() && !book.pages.is_empty() {
        return vec![(book.title.clone(), page_name(1))];
    }
    chapters
}

fn navigation(book: &Book) -> String {
    let entries: String = contents(book)
        .iter()
        .map(|(title, page)| format!("<li><a href=\"{page}\">{}</a></li>\n", escape(title)))
        .collect();
    xhtml(
        book,
        None,
        &format!(
            "<nav epub:type=\"toc\" id=\"toc\">\n<h1>{}</h1>\n<ol>\n{entries}</ol>\n</nav>",
            escape(&book.title)
        ),
    )
}

fn ncx(book: &Book) -> String {
    let points: String = contents(book)
        .iter()
        .enumerate()
        .map(|(index, (title, page))| {
            format!(
                "<navPoint id=\"point-{number}\" playOrder=\"{number}\"><navLabel><text>{}</text></navLabel><content \
                 src=\"{page}\"/></navPoint>\n",
                escape(title),
                number = index + 1
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" \
         version=\"2005-1\">\n<head><meta name=\"dtb:uid\" content=\"{}\"/></head>\n<docTitle><text>{}</text></docTitle>\n<navMap>\n{points}</navMap>\n</ncx>\n",
        escape(&book.identifier),
        escape(&book.title)
    )
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn book() -> Book {
        Book {
            identifier: "https://www.pixiv.net/novel/show.php?id=1".to_owned(),
            title: "A <Novel> & Co".to_owned(),
            author: Some("Someone".to_owned()),
            language: "ja".to_owned(),
            date: Some("2026-11-22".to_owned()),
            description: None,
            subjects: vec!["tag".to_owned()],
            cover: Some(Cover {
                bytes: vec![0xFF, 0xD8, 0xFF],
                media_type: "image/jpeg".to_owned(),
            }),
            pages: vec![
                Page {
                    title: Some("One".to_owned()),
                    body: "<h2>One</h2><p>Hello</p>".to_owned(),
                },
                Page {
                    title: None,
                    body: "<p>Again</p>".to_owned(),
                },
            ],
        }
    }

    #[test]
    fn writes_a_book_readers_open() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("book.epub");
        write(&book(), &path).unwrap();
        let mut zip = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
        {
            let first = zip.by_index(0).unwrap();
            assert_eq!(
                first.name().unwrap(),
                "mimetype",
                "first, as readers require"
            );
            assert_eq!(first.compression(), CompressionMethod::Stored);
        }
        let mut read = |name: &str| {
            let mut text = String::new();
            zip.by_name(name)
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            text
        };
        let package = read("OEBPS/content.opf");
        assert!(package.contains("<dc:title>A &lt;Novel&gt; &amp; Co</dc:title>"));
        assert!(package.contains("properties=\"cover-image\""));
        assert!(package.contains(
            "<itemref idref=\"cover\"/>\n<itemref idref=\"page-1\"/>\n<itemref idref=\"page-2\"/>"
        ));
        assert!(read("OEBPS/nav.xhtml").contains("<li><a href=\"page-0001.xhtml\">One</a></li>"));
        assert!(read("OEBPS/page-0002.xhtml").contains("<p>Again</p>"));
        assert!(!folder.path().join("book.epub.part").exists());
    }

    #[test]
    fn escapes_text_for_xml() {
        assert_eq!(
            escape("a < b & \"c\" 'd'\u{1}"),
            "a &lt; b &amp; &quot;c&quot; &#39;d&#39;"
        );
    }
}
