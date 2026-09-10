use jm_common::Disposition;

/// An obligation is a disposition together with the claim that introduced it.
/// The lattice lives on [`Disposition::join`]; this wrapper exists so the
/// warrant can name *who* raised the floor.
#[derive(Debug, Clone, PartialEq)]
pub struct Obligation {
    pub disposition: Disposition,
    pub by: String,
}

impl Obligation {
    pub fn show() -> Self {
        Self {
            disposition: Disposition::Show,
            by: "LeastCommitment".into(),
        }
    }

    pub fn join(self, other: Self) -> Self {
        if other.disposition.rank() > self.disposition.rank() {
            other
        } else {
            self
        }
    }
}
