use std::fmt;
use std::str::FromStr;

use uri::Uri;
use mime::Mime;
use language_tags::LanguageTag;

use header::{Header, Raw};
use header::parsing::from_one_raw_str;

/// The `Link` header, defined in
/// [RFC5988](http://tools.ietf.org/html/rfc5988#section-5)
///
/// # ABNF
/// ```plain
/// Link           = "Link" ":" #link-value
/// link-value     = "<" URI-Reference ">" *( ";" link-param )
/// link-param     = ( ( "rel" "=" relation-types )
///                | ( "anchor" "=" <"> URI-Reference <"> )
///                | ( "rev" "=" relation-types )
///                | ( "hreflang" "=" Language-Tag )
///                | ( "media" "=" ( MediaDesc | ( <"> MediaDesc <"> ) ) )
///                | ( "title" "=" quoted-string )
///                | ( "title*" "=" ext-value )
///                | ( "type" "=" ( media-type | quoted-mt ) )
///                | ( link-extension ) )
/// link-extension = ( parmname [ "=" ( ptoken | quoted-string ) ] )
///                | ( ext-name-star "=" ext-value )
/// ext-name-star  = parmname "*" ; reserved for RFC2231-profiled
/// ; extensions.  Whitespace NOT
/// ; allowed in between.
/// ptoken         = 1*ptokenchar
/// ptokenchar     = "!" | "#" | "$" | "%" | "&" | "'" | "("
///                | ")" | "*" | "+" | "-" | "." | "/" | DIGIT
///                | ":" | "<" | "=" | ">" | "?" | "@" | ALPHA
///                | "[" | "]" | "^" | "_" | "`" | "{" | "|"
///                | "}" | "~"
/// media-type     = type-name "/" subtype-name
/// quoted-mt      = <"> media-type <">
/// relation-types = relation-type
///                | <"> relation-type *( 1*SP relation-type ) <">
/// relation-type  = reg-rel-type | ext-rel-type
/// reg-rel-type   = LOALPHA *( LOALPHA | DIGIT | "." | "-" )
/// ext-rel-type   = URI
/// ```
///
/// # Example values
///
/// ...
///
/// # Examples
///
/// ...
///
///
#[derive(Clone, PartialEq, Debug)]
pub struct Link {
    /// Target IRI: `link-value`
    pub link: Uri,

    /// Forward Relation Types: `rel`
    pub rel: Option<Vec<RelationType>>,

    /// Context IRI: `anchor`
    pub anchor: Option<Uri>,

    /// Reverse Relation Types: `rev`
    pub rev: Option<Vec<RelationType>>,

    /// Language Tag: `hreflang`
    pub href_lang: Vec<LanguageTag>,

    /// Media Descriptors: `media`
    pub media_desc: Option<Vec<MediaDesc>>,

    /// Quoted String: `title`
    pub title: Option<String>,

    /// Extended Value: `title*`
    pub title_star: Option<String>,

    /// Media Type: `type`
    pub media_type: Option<Mime>,

    /// Link Extension: `link-extensions`
    pub link_extension: Option<String>
}

/// A Media Descriptors Enum based on
/// https://www.w3.org/TR/html401/types.html#h-6.13
#[derive(Clone, PartialEq, Debug)]
pub enum MediaDesc {
    /// screen
    Screen,
    /// tty
    Tty,
    /// tv
    Tv,
    /// projection
    Projection,
    /// handheld
    Handheld,
    /// print
    Print,
    /// braille
    Braille,
    /// aural
    Aural,
    /// all
    All,
    /// Other Values
    Value(String)
}

/// A Link Relation Type Enum based on
/// [RFC5988](https://tools.ietf.org/html/rfc5988#section-6.2.2)
#[derive(Clone, PartialEq, Debug)]
pub enum RelationType {
    /// alternate
    Alternate,
    /// appendix
    Appendix,
    /// bookmark
    Bookmark,
    /// chapter
    Chapter,
    /// contents
    Contents,
    /// copyright
    Copyright,
    /// current
    Current,
    /// describedby
    DescribedBy,
    /// edit
    Edit,
    /// editMedia
    EditMedia,
    /// enclosure
    Enclosure,
    /// first
    First,
    /// glossary
    Glossary,
    /// help
    Help,
    /// hub
    Hub,
    /// index
    Index,
    /// last
    Last,
    /// latestVersion
    LatestVersion,
    /// license
    License,
    /// next
    Next,
    /// nextArchive
    NextArchive,
    /// payment
    Payment,
    /// prev
    Prev,
    /// predecessorVersion
    PredecessorVersion,
    /// previous
    Previous,
    /// prevArchive
    PrevArchive,
    /// related
    Related,
    /// replies
    Replies,
    /// section
    Section,
    /// self
    RelationTypeSelf,
    /// service
    Service,
    /// start
    Start,
    /// stylesheet
    Stylesheet,
    /// subsection
    Subsection,
    /// successorVersion
    SuccessorVersion,
    /// up
    Up,
    /// versionHistory
    VersionHistory,
    /// via
    Via,
    /// working-copy
    WorkingCopy,
    /// working-copy-of
    WorkingCopyOf,
    /// ext-rel-type
    ExtRelType(Uri)
}

impl Link {
    /// Create Link from URI-Reference
    pub fn new(uri: &str) -> ::Result<Link> {
        match Uri::new(uri) {
            Err(_) => Err(::Error::Header),
            Ok(u) => Ok(Link {
                link: u,
                rel: None,
                anchor: None,
                rev: None,
                href_lang: Vec::new(),
                media_desc: None,
                title: None,
                title_star: None,
                media_type: None,
                link_extension: None,
            })
        }
    }
}

impl Header for Link {
    fn header_name() -> &'static str {
        static NAME: &'static str = "Link";
        NAME
    }

    fn parse_header(raw: &Raw) -> ::Result<Link> {
       from_one_raw_str(raw)
    }

    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: Implementation missing.
        write!(f, "<{}>", self.link)
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_header(f)
    }
}

impl FromStr for Link {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<Link> {
        let mut link_split = s.split(';');

        // Parse the `Target IRI`
        // https://tools.ietf.org/html/rfc5988#section-5.1
        let mut link_header = match link_split.next() {
            None => return Err(::Error::Header),
            Some(link_value) => match verify_and_trim(link_value, '<', '>') {
                Err(_) => return Err(::Error::Header),
                Ok(link_url) => match Uri::new(link_url) {
                    Err(_) => return Err(::Error::Header),
                    Ok(uri) => Link {
                        link: uri,
                        rel: None,
                        anchor: None,
                        rev: None,
                        href_lang: Vec::new(),
                        media_desc: None,
                        title: None,
                        title_star: None,
                        media_type: None,
                        link_extension: None,
                    }
                },
            },
        };

        // Parse the Link header's parameters
        for link_param in link_split {
            let mut link_param_split = link_param.split('=');

            let link_param_name = match link_param_split.next() {
                None => return Err(::Error::Header),
                Some(p) => p.trim(),
            };

            match link_param_name.to_lowercase().as_ref() {
                "rel" => {
                    // Parse relation type: `rel`.
                    // https://tools.ietf.org/html/rfc5988#section-5.3
                    if link_header.rel.is_none() {
                        link_header.rel = match link_param_split.next() {
                            None => return Err(::Error::Header),
                            Some(rel_str) => {
                                let rel_tokens = rel_str.trim_matches('"').split(' ');

                                let mut rel: Vec<RelationType> = Vec::new();

                                for token in rel_tokens {
                                    match RelationType::from_str(token) {
                                        Err(_) => return Err(::Error::Header),
                                        Ok(r) => rel.push(r),
                                    }
                                }

                                Some(rel)
                            },
                        }
                    }
                },
                "anchor" => {
                    // Parse the `Context IRI`.
                    // https://tools.ietf.org/html/rfc5988#section-5.2
                    link_header.anchor = match link_param_split.next() {
                        None => return Err(::Error::Header),
                        Some(anchor_str) => match verify_and_trim(anchor_str, '"', '"') {
                            Err(_) => return Err(::Error::Header),
                            Ok(anchor) => match Uri::new(anchor) {
                                Err(_) => return Err(::Error::Header),
                                Ok(uri) => Some(uri),
                            },
                        },
                    };
                },
                "rev" => {
                    // Parse relation type: `rev`.
                    // https://tools.ietf.org/html/rfc5988#section-5.3
                    if link_header.rev.is_none() {
                        link_header.rev = match link_param_split.next() {
                            None => return Err(::Error::Header),
                            Some(rev_str) => {
                                let rev_tokens = rev_str.trim_matches('"').split(' ');

                                let mut rev: Vec<RelationType> = Vec::new();

                                for token in rev_tokens {
                                    match RelationType::from_str(token) {
                                        Err(_) => return Err(::Error::Header),
                                        Ok(r) => rev.push(r),
                                    }
                                }

                                Some(rev)
                            },
                        }
                    }
                },
                "hreflang" => {
                    // Parse target attribute: `hreflang`.
                    // https://tools.ietf.org/html/rfc5988#section-5.4
                    let href_lang = match link_param_split.next() {
                        None => return Err(::Error::Header),
                        Some(t) => {
                            match t.parse::<LanguageTag>() {
                                Err(_) => return Err(::Error::Header),
                                Ok(lt) => lt,
                            }
                        }
                    };

                    link_header.href_lang.push(href_lang);
                },
                "media" => {
                    // Parse target attribute: `media`.
                    // https://tools.ietf.org/html/rfc5988#section-5.4
                    //
                    // TODO: Each entry is truncated just before the first
                    //       character that isn't a US ASCII letter [a-zA-Z]
                    //       (ISO 10646 hex 41-5a, 61-7a), digit [0-9]
                    //       (hex 30-39), or hyphen (hex 2d).
                    //
                    // TODO: Check with the `.unwrap()`.
                    if link_header.media_desc.is_none() {
                        let media_desc = match link_param_split.next() {
                            None => return Err(::Error::Header),
                            Some(desc_str) => {
                                desc_str.trim_matches('"')
                                        .split(',')
                                        .map(|d| MediaDesc::from_str(d.trim()).unwrap())
                                        .collect()
                            }
                        };

                        link_header.media_desc = Some(media_desc);
                    }
                },
                "title" => {
                    // Parse target attribute: `title`.
                    // https://tools.ietf.org/html/rfc5988#section-5.4
                    if link_header.title.is_none() {
                        link_header.title = match link_param_split.next() {
                            None => return Err(::Error::Header),
                            Some(title_str) => match verify_and_trim(title_str, '"', '"') {
                                Err(_) => return Err(::Error::Header),
                                Ok(title) => Some(String::from(title)),
                            },
                        };
                    }
                },
                "title*" => {
                    // Parse target attribute: `title*`.
                    // https://tools.ietf.org/html/rfc5988#section-5.4
                    if link_header.title_star.is_none() {
                        link_header.title_star = match link_param_split.next() {
                            None => return Err(::Error::Header),
                            Some(title_str) => Some(String::from(title_str)),
                        };
                    }
                },
                "type" => {
                    // Parse target attribute: `type`.
                    // https://tools.ietf.org/html/rfc5988#section-5.4
                    if link_header.media_type.is_none() {
                        link_header.media_type = match link_param_split.next() {
                            None => return Err(::Error::Header),
                            Some(media_type_str) => match verify_and_trim(media_type_str, '"', '"') {
                                Err(_) => return Err(::Error::Header),
                                Ok(media_type) => match media_type.parse::<Mime>() {
                                    Err(_) => return Err(::Error::Header),
                                    Ok(mime_type) => Some(mime_type),
                                },
                            },

                        };
                    }
                },
                "link-extension" => {
                    // Parse target attribute: `link-extension`.
                    // https://tools.ietf.org/html/rfc5988#section-5.4
                    if link_header.link_extension.is_none() {
                        link_header.link_extension = match link_param_split.next() {
                            None => return Err(::Error::Header),
                            Some(link_extension_str) => Some(String::from(link_extension_str)),
                        };
                    }
                },
                _ => {
                    return Err(::Error::Header);
                }
            }
        }

        Ok(link_header)
    }
}

fn verify_and_trim(s: &str, start: char, end: char) -> ::Result<&str> {
    let length = s.len();

    if length < 2 {
        return Err(::Error::Header);
    } else {
        let start_char = s.chars().nth(0).unwrap();
        let end_char = s.chars().nth(length - 1).unwrap();

        if start != start_char || end != end_char {
            return Err(::Error::Header);
        } else {
            return Ok(s.trim_matches(|c| c == start || c == end || c == ' '));
        }
    }
}

impl fmt::Display for MediaDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MediaDesc::Screen => write!(f, "screen"),
            MediaDesc::Tty => write!(f, "tty"),
            MediaDesc::Tv => write!(f, "tv"),
            MediaDesc::Projection => write!(f, "projection"),
            MediaDesc::Handheld => write!(f, "handheld"),
            MediaDesc::Print => write!(f, "print"),
            MediaDesc::Braille => write!(f, "braille"),
            MediaDesc::Aural => write!(f, "aural"),
            MediaDesc::All => write!(f, "all"),
            MediaDesc::Value(ref other) => write!(f, "{}", other),
         }
    }
}

impl FromStr for MediaDesc {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<MediaDesc> {
        match s {
            "screen" => Ok(MediaDesc::Screen),
            "tty" => Ok(MediaDesc::Tty),
            "tv" => Ok(MediaDesc::Tv),
            "projection" => Ok(MediaDesc::Projection),
            "handheld" => Ok(MediaDesc::Handheld),
            "print" => Ok(MediaDesc::Print),
            "braille" => Ok(MediaDesc::Braille),
            "aural" => Ok(MediaDesc::Aural),
            "all" => Ok(MediaDesc::All),
            _ => Ok(MediaDesc::Value(String::from(s))),
        }
    }
}

impl fmt::Display for RelationType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RelationType::Alternate => write!(f, "alternate"),
            RelationType::Appendix => write!(f, "appendix"),
            RelationType::Bookmark => write!(f, "bookmark"),
            RelationType::Chapter => write!(f, "chapter"),
            RelationType::Contents => write!(f, "contents"),
            RelationType::Copyright => write!(f, "copyright"),
            RelationType::Current => write!(f, "current"),
            RelationType::DescribedBy => write!(f, "describedby"),
            RelationType::Edit => write!(f, "edit"),
            RelationType::EditMedia => write!(f, "edit-media"),
            RelationType::Enclosure => write!(f, "enclosure"),
            RelationType::First => write!(f, "first"),
            RelationType::Glossary => write!(f, "glossary"),
            RelationType::Help => write!(f, "help"),
            RelationType::Hub => write!(f, "hub"),
            RelationType::Index => write!(f, "index"),
            RelationType::Last => write!(f, "last"),
            RelationType::LatestVersion => write!(f, "latest-version"),
            RelationType::License => write!(f, "license"),
            RelationType::Next => write!(f, "next"),
            RelationType::NextArchive => write!(f, "next-archive"),
            RelationType::Payment => write!(f, "payment"),
            RelationType::Prev => write!(f, "prev"),
            RelationType::PredecessorVersion => write!(f, "predecessor-version"),
            RelationType::Previous => write!(f, "previous"),
            RelationType::PrevArchive => write!(f, "prev-archive"),
            RelationType::Related => write!(f, "related"),
            RelationType::Replies => write!(f, "replies"),
            RelationType::Section => write!(f, "section"),
            RelationType::RelationTypeSelf => write!(f, "self"),
            RelationType::Service => write!(f, "service"),
            RelationType::Start => write!(f, "start"),
            RelationType::Stylesheet => write!(f, "stylesheet"),
            RelationType::Subsection => write!(f, "subsection"),
            RelationType::SuccessorVersion => write!(f, "successor-version"),
            RelationType::Up => write!(f, "up"),
            RelationType::VersionHistory => write!(f, "version-history"),
            RelationType::Via => write!(f, "via"),
            RelationType::WorkingCopy => write!(f, "working-copy"),
            RelationType::WorkingCopyOf => write!(f, "working-copy-of"),
            RelationType::ExtRelType(ref uri) => write!(f, "{}", uri),
         }
    }
}

impl FromStr for RelationType {
    type Err = ::Error;

    fn from_str(s: &str) -> ::Result<RelationType> {
        // TODO: Shouldn't we have to add a RegRelType based on:
        //       `reg-rel-type   = LOALPHA *( LOALPHA | DIGIT | "." | "-" )`

        match s {
            "alternate" => Ok(RelationType::Alternate),
            "appendix" => Ok(RelationType::Appendix),
            "bookmark" => Ok(RelationType::Bookmark),
            "chapter" => Ok(RelationType::Chapter),
            "contents" => Ok(RelationType::Contents),
            "copyright" => Ok(RelationType::Copyright),
            "current" => Ok(RelationType::Current),
            "describedby" => Ok(RelationType::DescribedBy),
            "edit" => Ok(RelationType::Edit),
            "edit-media" => Ok(RelationType::EditMedia),
            "enclosure" => Ok(RelationType::Enclosure),
            "first" => Ok(RelationType::First),
            "glossary" => Ok(RelationType::Glossary),
            "help" => Ok(RelationType::Help),
            "hub" => Ok(RelationType::Hub),
            "index" => Ok(RelationType::Index),
            "last" => Ok(RelationType::Last),
            "latest-version" => Ok(RelationType::LatestVersion),
            "license" => Ok(RelationType::License),
            "next" => Ok(RelationType::Next),
            "next-archive" => Ok(RelationType::NextArchive),
            "payment" => Ok(RelationType::Payment),
            "prev" => Ok(RelationType::Prev),
            "predecessor-version" => Ok(RelationType::PredecessorVersion),
            "previous" => Ok(RelationType::Previous),
            "prev-archive" => Ok(RelationType::PrevArchive),
            "related" => Ok(RelationType::Related),
            "replies" => Ok(RelationType::Replies),
            "section" => Ok(RelationType::Section),
            "self" => Ok(RelationType::RelationTypeSelf),
            "service" => Ok(RelationType::Service),
            "start" => Ok(RelationType::Start),
            "stylesheet" => Ok(RelationType::Stylesheet),
            "subsection" => Ok(RelationType::Subsection),
            "successor-version" => Ok(RelationType::SuccessorVersion),
            "up" => Ok(RelationType::Up),
            "version-history" => Ok(RelationType::VersionHistory),
            "via" => Ok(RelationType::Via),
            "working-copy" => Ok(RelationType::WorkingCopy),
            "working-copy-of" => Ok(RelationType::WorkingCopyOf),
            _ => match Uri::new(s) {
                Err(_) => Err(::Error::Header),
                Ok(uri) => Ok(RelationType::ExtRelType(uri)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Link, RelationType};
    use header::Header;

    #[test]
    fn test_link() {
        let uri = "http://example.com/TheBook/chapter2";
        let link_header = b"<http://example.com/TheBook/chapter2>; rel=\"previous\"; rev=next; title=\"previous chapter\"";

        let mut expected_link = Link::new(uri).unwrap();

        expected_link.rel = Some(vec![RelationType::Previous]);
        expected_link.rev = Some(vec![RelationType::Next]);

        expected_link.title = Some(String::from("previous chapter"));

        let link = Header::parse_header(&vec![link_header.to_vec()].into());
        assert_eq!(link.ok(), Some(expected_link));
    }
}
