#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TvdbId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TmdbId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TvSeriesId {
    pub tvdb_id: TvdbId,
    pub tmdb_bridge: Option<TmdbId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MovieId {
    pub tmdb_id: TmdbId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaId {
    Tv(TvSeriesId),
    Movie(MovieId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_identity_is_tvdb_canonical_with_optional_tmdb_bridge() {
        let id = MediaId::Tv(TvSeriesId {
            tvdb_id: TvdbId(76666),
            tmdb_bridge: Some(TmdbId(12609)),
        });
        match id {
            MediaId::Tv(tv) => {
                assert_eq!(tv.tvdb_id.0, 76666);
                assert_eq!(tv.tmdb_bridge.unwrap().0, 12609);
            }
            _ => panic!("expected tv id"),
        }
    }

    #[test]
    fn movie_identity_is_tmdb_canonical() {
        let id = MediaId::Movie(MovieId {
            tmdb_id: TmdbId(550),
        });
        assert!(matches!(
            id,
            MediaId::Movie(MovieId {
                tmdb_id: TmdbId(550)
            })
        ));
    }
}
