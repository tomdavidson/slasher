use arbitrary::Arbitrary;

pub const MAX_FRAGMENTS: usize = 20;
pub const MAX_JOIN_PARTS: usize = 5;
pub const MAX_BODY_LINES: usize = 20;
pub const MAX_CMD_NAME_LEN: usize = 30;

#[derive(Debug)]
pub struct FuzzDoc {
    pub fragments: Vec<Fragment>,
}

impl<'a> Arbitrary<'a> for FuzzDoc {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let len = u.int_in_range(0..=MAX_FRAGMENTS)?;
        let fragments = (0..len)
            .map(|_| Fragment::arbitrary(u))
            .collect::<arbitrary::Result<Vec<_>>>()?;
        Ok(FuzzDoc { fragments })
    }
}

#[derive(Debug)]
pub enum Fragment {
    Text(TextLine),
    SingleLineCmd(CmdName, Payload),
    FencedCmd(CmdName, Header, FenceLang, FenceBody),
    UnclosedFence(CmdName, Header, FenceBody),
    JoinedCmd(CmdName, Vec<Payload>),
    InvalidSlash(InvalidSlashKind),
    Blank,
}

impl<'a> Arbitrary<'a> for Fragment {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let variant = u.int_in_range(0..=6u8)?;
        Ok(match variant {
            0 => Fragment::Text(u.arbitrary()?),
            1 => Fragment::SingleLineCmd(u.arbitrary()?, u.arbitrary()?),
            2 => Fragment::FencedCmd(
                u.arbitrary()?,
                u.arbitrary()?,
                u.arbitrary()?,
                u.arbitrary()?,
            ),
            3 => Fragment::UnclosedFence(u.arbitrary()?, u.arbitrary()?, u.arbitrary()?),
            4 => {
                let name: CmdName = u.arbitrary()?;
                let len = u.int_in_range(0..=MAX_JOIN_PARTS)?;
                let parts = (0..len)
                    .map(|_| Payload::arbitrary(u))
                    .collect::<arbitrary::Result<Vec<_>>>()?;
                Fragment::JoinedCmd(name, parts)
            }
            5 => Fragment::InvalidSlash(u.arbitrary()?),
            _ => Fragment::Blank,
        })
    }
}

#[derive(Debug)]
pub struct CmdName {
    pub raw: Vec<u8>,
}

impl<'a> Arbitrary<'a> for CmdName {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let len = u.int_in_range(0..=MAX_CMD_NAME_LEN)?;
        let raw = (0..len)
            .map(|_| u8::arbitrary(u))
            .collect::<arbitrary::Result<Vec<_>>>()?;
        Ok(CmdName { raw })
    }
}

#[derive(Arbitrary, Debug)]
pub struct TextLine {
    pub content: String,
}

#[derive(Arbitrary, Debug)]
pub struct Payload {
    pub text: String,
}

#[derive(Arbitrary, Debug)]
pub struct Header {
    pub text: String,
}

#[derive(Arbitrary, Debug)]
pub struct FenceLang {
    pub lang: Option<String>,
}

#[derive(Debug)]
pub struct FenceBody {
    pub lines: Vec<String>,
}

impl<'a> Arbitrary<'a> for FenceBody {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let len = u.int_in_range(0..=MAX_BODY_LINES)?;
        let lines = (0..len)
            .map(|_| String::arbitrary(u))
            .collect::<arbitrary::Result<Vec<_>>>()?;
        Ok(FenceBody { lines })
    }
}

#[derive(Arbitrary, Debug)]
pub enum InvalidSlashKind {
    BareSlash,
    NumericAfterSlash,
    Capitalized,
    TrailingHyphen,
}
