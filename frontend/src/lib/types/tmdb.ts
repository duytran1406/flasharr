export interface TMDBMovie {
  id: number;
  title: string;
  poster_path?: string;
  backdrop_path?: string;
  overview: string;
  release_date: string;
  vote_average: number;
  genre_ids: number[];
}

export interface TMDBTVShow {
  id: number;
  name: string;
  poster_path?: string;
  backdrop_path?: string;
  overview: string;
  first_air_date: string;
  vote_average: number;
  genre_ids: number[];
}
