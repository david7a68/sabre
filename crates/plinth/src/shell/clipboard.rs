use tracing::error;

pub struct Clipboard {
    inner: arboard::Clipboard,
}

impl Clipboard {
    pub(crate) fn new() -> Self {
        Self {
            inner: arboard::Clipboard::new().unwrap(),
        }
    }

    pub fn get_text(&mut self) -> Option<String> {
        match self.inner.get_text() {
            Ok(text) => Some(text),
            Err(error) => {
                error!(%error, "Unable to get clipboard text");
                None
            }
        }
    }

    pub fn set_text(&mut self, text: &str) {
        if let Err(error) = self.inner.set_text(text) {
            error!(%error, "Unable to set clipboard text");
        }
    }
}
