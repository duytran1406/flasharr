/// TMDB API key loaded from the process environment.
pub fn tmdb_api_key() -> String {
    std::env::var("TMDB_API_KEY").unwrap_or_default()
}
