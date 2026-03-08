#[derive(Debug, Clone, Default)]
pub struct ApiParams {
    pub path_segments: Vec<String>,
    pub query_params: Vec<(String, String)>,
}

impl ApiParams {
    pub fn new(path_segments: Vec<impl Into<String>>) -> Self {
        Self {
            path_segments: path_segments.into_iter().map(Into::into).collect(),
            query_params: Vec::new(),
        }
    }

    pub fn with_query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.push((key.into(), value.into()));
        self
    }
}
