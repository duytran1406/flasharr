"""
Test suite for filename parser
"""

import pytest
from app.filename_parser import FilenameNormalizer


class TestFilenameNormalizer:
    """Test cases for FilenameNormalizer"""
    
    def setup_method(self):
        """Setup test fixtures"""
        self.normalizer = FilenameNormalizer()
    
    def test_problematic_vietnamese_file(self):
        """Test the original problematic file"""
        filename = "Ling Cage 2019 4K HDR 10Bit S1E14 SP TVP TMPĐ_kimngonx5 (2019) 2160p"
        parsed = self.normalizer.parse(filename)
        
        assert parsed.is_series == True
        assert parsed.title == "Ling Cage"
        assert parsed.season == 1
        assert parsed.episode == 14
        assert parsed.year == 2019
        assert "4K" not in parsed.title
        assert "HDR" not in parsed.title
        assert "10Bit" not in parsed.title
    
    def test_standard_series_format(self):
        """Test standard series format"""
        filename = "The Mandalorian S02E08 1080p WEB-DL x264-RARBG"
        parsed = self.normalizer.parse(filename)
        
        assert parsed.is_series == True
        assert parsed.title == "The Mandalorian"
        assert parsed.season == 2
        assert parsed.episode == 8
    
    def test_movie_format(self):
        """Test movie format (no season/episode)"""
        filename = "Avengers Endgame (2019) 2160p BluRay x265 10bit HDR-RARBG"
        parsed = self.normalizer.parse(filename)
        
        assert parsed.is_series == False
        assert "Avengers Endgame" in parsed.title
    
    def test_quality_markers_moved(self):
        """Test that quality markers are moved after episode"""
        filename = "Show 4K HDR S01E01 2160p"
        parsed = self.normalizer.parse(filename)
        
        # Title should not contain quality markers
        assert "4K" not in parsed.title
        assert "HDR" not in parsed.title
        
        # Normalized filename should have quality after episode
        assert "S01E01" in parsed.normalized_filename
        title_end = parsed.normalized_filename.index("S01E01")
        quality_part = parsed.normalized_filename[title_end:]
        assert "4K" in quality_part
        assert "HDR" in quality_part
    
    def test_vietnamese_markers(self):
        """Test Vietnamese-specific markers"""
        filename = "Squid Game 2021 Vietsub S01E01 1080p"
        parsed = self.normalizer.parse(filename)
        
        assert parsed.title == "Squid Game"
        assert "Vietsub" not in parsed.title
        assert parsed.season == 1
        assert parsed.episode == 1
    
    def test_season_episode_formats(self):
        """Test various season/episode formats"""
        test_cases = [
            ("Show S01E01", 1, 1),
            ("Show S1E1", 1, 1),
            ("Show S01 E01", 1, 1),
            ("Show S01EP01", 1, 1),
        ]
        
        for filename, expected_season, expected_episode in test_cases:
            parsed = self.normalizer.parse(filename)
            assert parsed.season == expected_season, f"Failed for {filename}"
            assert parsed.episode == expected_episode, f"Failed for {filename}"
    
    def test_file_extensions(self):
        """Test that file extensions are preserved"""
        filename = "Show S01E01.mkv"
        parsed = self.normalizer.parse(filename)
        
        assert parsed.normalized_filename.endswith(".mkv")
    
    def test_year_extraction(self):
        """Test year extraction"""
        filename = "Show 2020 S01E01"
        parsed = self.normalizer.parse(filename)
        
        assert parsed.year == 2020
        assert "2020" not in parsed.title


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
