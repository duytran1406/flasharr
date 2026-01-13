"""
Unit tests for the formatters module.
"""

import pytest
from src.flasharr.utils.formatters import (
    format_size,
    format_speed,
    format_duration,
    format_eta,
)


class TestFormatSize:
    """Tests for format_size function."""
    
    def test_zero_bytes(self):
        assert format_size(0) == "0 B"
    
    def test_bytes(self):
        assert format_size(512) == "512.00 B"
    
    def test_kilobytes(self):
        assert format_size(1024) == "1.00 KB"
        assert format_size(1536) == "1.50 KB"
    
    def test_megabytes(self):
        assert format_size(1048576) == "1.00 MB"
        assert format_size(1572864) == "1.50 MB"
    
    def test_gigabytes(self):
        assert format_size(1073741824) == "1.00 GB"
        assert format_size(4831838208) == "4.50 GB"
    
    def test_terabytes(self):
        assert format_size(1099511627776) == "1.00 TB"
    
    def test_custom_precision(self):
        assert format_size(1536, precision=1) == "1.5 KB"
        assert format_size(1536, precision=0) == "2 KB"
        assert format_size(1536, precision=3) == "1.500 KB"
    
    def test_float_input(self):
        assert format_size(1024.5) == "1.00 KB"


class TestFormatSpeed:
    """Tests for format_speed function."""
    
    def test_zero_speed(self):
        assert format_speed(0) == "0 B/s"
    
    def test_bytes_per_second(self):
        assert format_speed(512) == "512.00 B/s"
    
    def test_kilobytes_per_second(self):
        assert format_speed(1024) == "1.00 KB/s"
    
    def test_megabytes_per_second(self):
        assert format_speed(10485760) == "10.00 MB/s"
    
    def test_custom_precision(self):
        assert format_speed(1536, precision=1) == "1.5 KB/s"


class TestFormatDuration:
    """Tests for format_duration function."""
    
    def test_zero_seconds(self):
        assert format_duration(0) == "0s"
    
    def test_negative_seconds(self):
        assert format_duration(-10) == "0s"
    
    def test_seconds_only(self):
        assert format_duration(45) == "45s"
    
    def test_minutes_and_seconds(self):
        assert format_duration(125) == "2m 5s"
    
    def test_hours_minutes_seconds(self):
        assert format_duration(3661) == "1h 1m 1s"
    
    def test_days(self):
        assert format_duration(90061) == "1d 1h 1m 1s"
    
    def test_exact_hour(self):
        assert format_duration(3600) == "1h"
    
    def test_float_input(self):
        assert format_duration(65.7) == "1m 5s"


class TestFormatEta:
    """Tests for format_eta function."""
    
    def test_zero_eta(self):
        assert format_eta(0) == "--:--"
    
    def test_negative_eta(self):
        assert format_eta(-10) == "--:--"
    
    def test_seconds_only(self):
        assert format_eta(45) == "00:45"
    
    def test_minutes_and_seconds(self):
        assert format_eta(125) == "02:05"
    
    def test_hours_minutes_seconds(self):
        assert format_eta(3661) == "01:01:01"
    
    def test_large_eta(self):
        assert format_eta(36000) == "10:00:00"
