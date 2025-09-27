from typing import Dict, Any, Tuple, Protocol, List, Optional

class ScoreAdapter(Protocol):
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        ...
    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        ...
    # Optional: some adapters expose full normalization
    def map_matches(self, raw_matches: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        return raw_matches

def _alliances_by_color_safe(match: Dict[str, Any]) -> Optional[Dict[str, Dict[str, Any]]]:
    """
    Try to return {'RED': {...}, 'BLUE': {...}}.
    If colors are missing, fall back to positional [BLUE, RED].
    Return None if we can't confidently provide both sides.
    """
    al = match.get("alliances") or []
    by_color: Dict[str, Dict[str, Any]] = {}

    # Prefer explicit labels if present
    for a in al:
        color = str(a.get("alliance", "")).upper()
        if color in ("RED", "BLUE"):
            by_color[color] = a

    # Fallback to positional order if labels missing
    if "BLUE" not in by_color and len(al) > 0:
        by_color["BLUE"] = al[0]
    if "RED" not in by_color and len(al) > 1:
        by_color["RED"] = al[1]

    if "RED" in by_color and "BLUE" in by_color:
        return by_color
    return None


class DefaultModernAdapter:
    """
    Seasons exposing teleop park/ascent and 'foulPointsCommitted' per alliance.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        by = _alliances_by_color_safe(match)
        if not by:
            return (0, 0)
        blue = int(by["BLUE"].get("teleopParkPoints", 0)) + int(by["BLUE"].get("teleopAscentPoints", 0))
        red  = int(by["RED"].get("teleopParkPoints", 0)) + int(by["RED"].get("teleopAscentPoints", 0))
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        by = _alliances_by_color_safe(match)
        if not by:
            return (0, 0)
        # points committed by each alliance (awarded to opponent)
        red  = int(by["RED"].get("foulPointsCommitted", 0))
        blue = int(by["BLUE"].get("foulPointsCommitted", 0))
        return red, blue


class Skystone2019Adapter:
    """
    2019 exposes:
      ... endGamePoints, penaltyPoints, totalPoints, autonomousPoints, etc.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        by = _alliances_by_color_safe(match)
        if not by:
            return (0, 0)
        red  = int(by["RED"].get("endGamePoints", 0))
        blue = int(by["BLUE"].get("endGamePoints", 0))
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        by = _alliances_by_color_safe(match)
        if not by:
            return (0, 0)
        red  = int(by["RED"].get("penaltyPoints", 0))   # points awarded to BLUE
        blue = int(by["BLUE"].get("penaltyPoints", 0))  # points awarded to RED
        return red, blue

    def map_matches(self, raw_matches: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        normalized: List[Dict[str, Any]] = []
        for raw_match in raw_matches:
            by = _alliances_by_color_safe(raw_match)
            if not by:
                # skip malformed or incomplete matches
                continue
            red, blue = by["RED"], by["BLUE"]

            normalized.append({
                "description": f'{str(raw_match.get("matchLevel","")).title()} {raw_match.get("matchNumber","")}',
                "scoreRedFinal": int(red.get("totalPoints", 0)),
                "scoreBlueFinal": int(blue.get("totalPoints", 0)),
                "scoreRedAuto": int(red.get("autonomousPoints", 0)),
                "scoreBlueAuto": int(blue.get("autonomousPoints", 0)),
                "scoreRedEndgame": int(red.get("endGamePoints", 0)),
                "scoreBlueEndgame": int(blue.get("endGamePoints", 0)),
                # "foul points committed by X" == points awarded to opponent
                "scoreRedFoul": int(red.get("penaltyPoints", 0)),     # later subtracted from BLUE teleop
                "scoreBlueFoul": int(blue.get("penaltyPoints", 0)),   # later subtracted from RED teleop
                # If teams roster isn’t in this payload, fill it elsewhere; MatrixBuilder tolerates empty list.
                "teams": raw_match.get("teams", []),
            })
        return normalized


SCORE_ADAPTERS: Dict[int, ScoreAdapter] = {
    2019: Skystone2019Adapter(),
    # 202x: SomeOtherAdapter(),
}
DEFAULT_SCORE_ADAPTER: ScoreAdapter = DefaultModernAdapter()