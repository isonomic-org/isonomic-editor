/// The kinds of inline formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkKind {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Link,
    Highlight,
    Color,
}

impl MarkKind {
    pub const ALL: [MarkKind; 8] = [
        Self::Bold,
        Self::Italic,
        Self::Underline,
        Self::Strike,
        Self::Code,
        Self::Link,
        Self::Highlight,
        Self::Color,
    ];

    /// The stable name used in every serialized form.
    pub fn key(self) -> &'static str {
        match self {
            Self::Bold => "bold",
            Self::Italic => "italic",
            Self::Underline => "underline",
            Self::Strike => "strike",
            Self::Code => "code",
            Self::Link => "link",
            Self::Highlight => "highlight",
            Self::Color => "color",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    /// Whether text typed at the mark's trailing edge takes the mark.
    pub fn expands(self) -> bool {
        !matches!(self, Self::Code | Self::Link)
    }

    /// Whether the mark carries a value (a URL or a color name).
    pub fn has_value(self) -> bool {
        matches!(self, Self::Link | Self::Highlight | Self::Color)
    }
}

/// One mark with its value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Link(String),
    Highlight(String),
    Color(String),
}

impl Mark {
    pub fn kind(&self) -> MarkKind {
        match self {
            Self::Bold => MarkKind::Bold,
            Self::Italic => MarkKind::Italic,
            Self::Underline => MarkKind::Underline,
            Self::Strike => MarkKind::Strike,
            Self::Code => MarkKind::Code,
            Self::Link(_) => MarkKind::Link,
            Self::Highlight(_) => MarkKind::Highlight,
            Self::Color(_) => MarkKind::Color,
        }
    }

    pub fn value(&self) -> Option<&str> {
        match self {
            Self::Link(v) | Self::Highlight(v) | Self::Color(v) => Some(v),
            _ => None,
        }
    }

    /// Builds a mark from its kind and optional value.
    pub fn new(kind: MarkKind, value: Option<&str>) -> Self {
        let value = value.unwrap_or_default().to_owned();
        match kind {
            MarkKind::Bold => Self::Bold,
            MarkKind::Italic => Self::Italic,
            MarkKind::Underline => Self::Underline,
            MarkKind::Strike => Self::Strike,
            MarkKind::Code => Self::Code,
            MarkKind::Link => Self::Link(value),
            MarkKind::Highlight => Self::Highlight(value),
            MarkKind::Color => Self::Color(value),
        }
    }
}

/// The set of marks on a run of text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Marks {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub code: bool,
    pub link: Option<String>,
    pub highlight: Option<String>,
    pub color: Option<String>,
}

impl Marks {
    pub fn with(mut self, mark: Mark) -> Self {
        self.set(&mark);
        self
    }

    pub fn set(&mut self, mark: &Mark) {
        match mark {
            Mark::Bold => self.bold = true,
            Mark::Italic => self.italic = true,
            Mark::Underline => self.underline = true,
            Mark::Strike => self.strike = true,
            Mark::Code => self.code = true,
            Mark::Link(v) => self.link = Some(v.clone()),
            Mark::Highlight(v) => self.highlight = Some(v.clone()),
            Mark::Color(v) => self.color = Some(v.clone()),
        }
    }

    pub fn clear(&mut self, kind: MarkKind) {
        match kind {
            MarkKind::Bold => self.bold = false,
            MarkKind::Italic => self.italic = false,
            MarkKind::Underline => self.underline = false,
            MarkKind::Strike => self.strike = false,
            MarkKind::Code => self.code = false,
            MarkKind::Link => self.link = None,
            MarkKind::Highlight => self.highlight = None,
            MarkKind::Color => self.color = None,
        }
    }

    pub fn get(&self, kind: MarkKind) -> Option<Mark> {
        let on = |flag: bool, mark: Mark| flag.then_some(mark);
        match kind {
            MarkKind::Bold => on(self.bold, Mark::Bold),
            MarkKind::Italic => on(self.italic, Mark::Italic),
            MarkKind::Underline => on(self.underline, Mark::Underline),
            MarkKind::Strike => on(self.strike, Mark::Strike),
            MarkKind::Code => on(self.code, Mark::Code),
            MarkKind::Link => self.link.clone().map(Mark::Link),
            MarkKind::Highlight => self.highlight.clone().map(Mark::Highlight),
            MarkKind::Color => self.color.clone().map(Mark::Color),
        }
    }

    pub fn has(&self, kind: MarkKind) -> bool {
        self.get(kind).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = Mark> + '_ {
        MarkKind::ALL.into_iter().filter_map(|kind| self.get(kind))
    }

    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }

    /// The marks that typing at this run's trailing edge inherits.
    pub fn expanding(&self) -> Self {
        let mut out = self.clone();
        for kind in MarkKind::ALL {
            if !kind.expands() {
                out.clear(kind);
            }
        }
        out
    }
}
