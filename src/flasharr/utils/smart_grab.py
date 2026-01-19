"""
Smart Grab Module

Intelligently selects the best complete season pack by analyzing:
- Release groups (e.g., NTb, C-ZONE, PHIM MEDIA)
- Quality consistency within groups
- File completeness
- Smart gap filling with best alternatives
"""

import re
from typing import List, Dict, Optional, Tuple
from collections import defaultdict
import statistics


def extract_release_group(filename: str) -> str:
    """
    Extract release group from filename.
    
    Common patterns:
    - Suffix after dash: "...-NTb.mkv" -> "NTb"
    - Bracketed: "...[C-ZONE].mkv" -> "C-ZONE"
    - Dotted: "...PHIM.MEDIA.mkv" -> "PHIM.MEDIA"
    
    Args:
        filename: Original filename
        
    Returns:
        Release group name or "Unknown"
    """
    # Remove extension
    name = re.sub(r'\.(mkv|mp4|avi)$', '', filename, flags=re.IGNORECASE)
    
    # Pattern 1: Bracketed group [GROUP]
    bracket_match = re.search(r'\[([A-Z0-9\-\.]+)\]', name, re.IGNORECASE)
    if bracket_match:
        return bracket_match.group(1)
    
    # Pattern 2: Suffix after dash -GROUP
    dash_match = re.search(r'-([A-Z0-9]+)$', name, re.IGNORECASE)
    if dash_match:
        group = dash_match.group(1)
        # Filter out quality indicators
        if group.upper() not in ['X264', 'X265', 'H264', 'H265', 'HEVC', 'AVC']:
            return group
    
    # Pattern 3: Dotted suffix PHIM.MEDIA or similar
    dotted_match = re.search(r'\.([A-Z]+\.[A-Z]+)$', name, re.IGNORECASE)
    if dotted_match:
        return dotted_match.group(1)
    
    return "Unknown"


def group_by_release_group(episodes: List[Dict]) -> Dict[str, Dict[int, Dict]]:
    """
    Group episode files by release group.
    
    Args:
        episodes: List of episode dicts with 'files' array
        
    Returns:
        Dict mapping release_group -> {episode_number -> best_file}
    """
    groups = defaultdict(dict)
    
    for episode in episodes:
        ep_num = episode.get('episode_number')
        if not ep_num:
            continue
            
        files = episode.get('files', [])
        if not files:
            continue
        
        # For each file, extract release group and store best per group
        for file in files:
            group = extract_release_group(file.get('name', ''))
            
            # Keep best file per episode per group (by normalized_score)
            if ep_num not in groups[group] or \
               file.get('normalized_score', 0) > groups[group][ep_num].get('normalized_score', 0):
                groups[group][ep_num] = file
    
    return dict(groups)


def calculate_group_score(
    group_files: Dict[int, Dict],
    total_episodes: int,
    expected_quality: Optional[str] = None
) -> Dict:
    """
    Score a release group by completeness, quality consistency, and average score.
    
    Args:
        group_files: Dict of episode_number -> file
        total_episodes: Total episodes in season
        expected_quality: Optional quality tier to match (e.g., "2160p")
        
    Returns:
        Score dict with metrics
    """
    found_episodes = len(group_files)
    completeness = found_episodes / total_episodes if total_episodes > 0 else 0
    
    # Quality metrics
    scores = [f.get('normalized_score', 0) for f in group_files.values()]
    quality_avg = statistics.mean(scores) if scores else 0
    quality_std = statistics.stdev(scores) if len(scores) > 1 else 0
    
    # Quality consistency bonus (lower std = more consistent)
    consistency_score = max(0, 100 - quality_std * 2)
    
    # Resolution consistency
    resolutions = [f.get('resolution', 'SD') for f in group_files.values()]
    resolution_consistency = len(set(resolutions)) == 1
    
    # Total score: weighted combination
    total_score = (
        completeness * 100 * 0.5 +  # 50% weight on completeness
        quality_avg * 0.3 +           # 30% weight on quality
        consistency_score * 0.2       # 20% weight on consistency
    )
    
    return {
        'completeness': completeness,
        'found_episodes': found_episodes,
        'quality_avg': quality_avg,
        'quality_std': quality_std,
        'consistency_score': consistency_score,
        'resolution_consistency': resolution_consistency,
        'total_score': total_score
    }


def select_best_alternative(
    candidates: List[Dict],
    primary_quality: str,
    primary_avg_score: float,
    primary_avg_size: float
) -> Dict:
    """
    Select best alternative file for missing episode.
    
    Priority:
    1. Same quality tier
    2. Best normalized score
    3. Similar file size
    
    Args:
        candidates: List of candidate files
        primary_quality: Primary group's quality (e.g., "2160p")
        primary_avg_score: Primary group's average score
        primary_avg_size: Primary group's average file size
        
    Returns:
        Best candidate file
    """
    if not candidates:
        return None
    
    # Score each candidate
    scored = []
    for candidate in candidates:
        score = 0
        
        # Quality match (40 points)
        if candidate.get('resolution') == primary_quality:
            score += 40
        
        # Normalized score (30 points)
        norm_score = candidate.get('normalized_score', 0)
        score += (norm_score / 100) * 30
        
        # Size similarity (30 points)
        size = candidate.get('size', 0)
        if primary_avg_size > 0:
            size_ratio = min(size, primary_avg_size) / max(size, primary_avg_size)
            score += size_ratio * 30
        
        scored.append((score, candidate))
    
    # Return best
    scored.sort(key=lambda x: x[0], reverse=True)
    return scored[0][1]


def smart_grab_season(season_data: Dict) -> Dict:
    """
    Main Smart Grab algorithm.
    
    Analyzes season data and returns optimal file selection for complete season.
    
    Args:
        season_data: Season dict with 'episodes_grouped' array
        
    Returns:
        Dict with:
        - urls: List of file URLs in episode order
        - metadata: Selection metadata
    """
    episodes = season_data.get('episodes_grouped', [])
    if not episodes:
        return {'urls': [], 'metadata': {'error': 'No episodes found'}}
    
    # Determine total episodes
    episode_numbers = [ep.get('episode_number') for ep in episodes if ep.get('episode_number')]
    if not episode_numbers:
        return {'urls': [], 'metadata': {'error': 'No valid episode numbers'}}
    
    total_episodes = max(episode_numbers)
    
    # Group by release group
    groups = group_by_release_group(episodes)
    
    if not groups:
        return {'urls': [], 'metadata': {'error': 'No release groups found'}}
    
    # Score each group
    scored_groups = []
    for group_name, group_files in groups.items():
        score = calculate_group_score(group_files, total_episodes)
        scored_groups.append({
            'name': group_name,
            'files': group_files,
            **score
        })
    
    # Sort by total_score DESC
    scored_groups.sort(key=lambda g: g['total_score'], reverse=True)
    
    # Select primary group
    primary = scored_groups[0]
    selected_files = dict(primary['files'])  # Copy
    
    # Calculate primary group stats for gap filling
    primary_quality = None
    primary_sizes = [f.get('size', 0) for f in primary['files'].values()]
    primary_avg_size = statistics.mean(primary_sizes) if primary_sizes else 0
    
    # Determine primary quality (most common resolution)
    if primary['files']:
        resolutions = [f.get('resolution', 'SD') for f in primary['files'].values()]
        primary_quality = max(set(resolutions), key=resolutions.count)
    
    # Fill missing episodes
    filled_from_others = 0
    quality_breakdown = defaultdict(int)
    
    for ep_num in range(1, total_episodes + 1):
        if ep_num in selected_files:
            # Count primary group
            group = extract_release_group(selected_files[ep_num].get('name', ''))
            quality_breakdown[group] += 1
        else:
            # Find best alternative
            candidates = []
            for group in scored_groups[1:]:
                if ep_num in group['files']:
                    candidates.append(group['files'][ep_num])
            
            if candidates:
                best = select_best_alternative(
                    candidates,
                    primary_quality,
                    primary['quality_avg'],
                    primary_avg_size
                )
                if best:
                    selected_files[ep_num] = best
                    filled_from_others += 1
                    group = extract_release_group(best.get('name', ''))
                    quality_breakdown[group] += 1
    
    # Build URL list in episode order
    urls = []
    for ep_num in sorted(selected_files.keys()):
        urls.append(selected_files[ep_num].get('url'))
    
    # Metadata
    metadata = {
        'total_episodes': total_episodes,
        'found_episodes': len(selected_files),
        'primary_group': primary['name'],
        'mixed_sources': filled_from_others > 0,
        'filled_from_others': filled_from_others,
        'quality_breakdown': dict(quality_breakdown),
        'primary_completeness': primary['completeness'],
        'primary_quality_avg': round(primary['quality_avg'], 1)
    }
    
    return {
        'urls': urls,
        'metadata': metadata
    }
