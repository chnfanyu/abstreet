use crate::GeomBatch;
use std::cell::RefCell;
use std::collections::HashMap;
use usvg::Options;

// TODO We don't need refcell maybe? Can we take &mut Assets?
pub struct Assets {
    pub default_line_height: f64,
    pub font_size: usize,
    text_cache: RefCell<HashMap<String, GeomBatch>>,
    pub text_opts: Options,
}

impl Assets {
    pub fn new(font_size: usize, font_dir: String) -> Assets {
        let mut a = Assets {
            default_line_height: 0.0,
            font_size,
            text_cache: RefCell::new(HashMap::new()),
            text_opts: Options::default(),
        };
        a.default_line_height = a.line_height(a.font_size);
        a.text_opts.font_directories.push(font_dir);
        a
    }

    pub fn line_height(&self, font_size: usize) -> f64 {
        // TODO Ahhh this stops working.
        font_size as f64
    }

    pub fn get_cached_text(&self, key: &str) -> Option<GeomBatch> {
        self.text_cache.borrow().get(key).cloned()
    }

    pub fn cache_text(&self, key: String, geom: GeomBatch) {
        self.text_cache.borrow_mut().insert(key, geom);
        //println!("cache has {} things",
        // abstutil::prettyprint_usize(self.text_cache.borrow().len()));
    }
}
