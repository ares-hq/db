from typing import Dict, Any, Tuple, Protocol, List

class ScoreAdapter(Protocol):
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        ...
    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        ...
    def map_matches(self, raw_matches: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        return raw_matches

class DefaultModernAdapter:
    """
    Seasons exposing teleop park/ascent and 'foulPointsCommitted' per alliance.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["teleopParkPoints"] + match["alliances"][1]["teleopAscentPoints"]
        blue = match["alliances"][0]["teleopParkPoints"] + match["alliances"][0]["teleopAscentPoints"]
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["foulPointsCommitted"]
        blue = match["alliances"][0]["foulPointsCommitted"]
        return red, blue

class Skystone2019Adapter:
    """
    2019 exposes:
      ... endGamePoints, penaltyPoints, totalPoints, autonomousPoints, etc.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["parkingPoints"] + match["alliances"][1]["capstonePoints"]
        blue = match["alliances"][0]["parkingPoints"] + match["alliances"][0]["capstonePoints"]
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["penaltyPoints"]
        blue = match["alliances"][0]["penaltyPoints"]
        return red, blue

class UltimateGoal2020Adapter:
    """
    2020 exposes:
      ... endGamePoints, penaltyPoints, totalPoints, autonomousPoints, etc.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["endgamePoints"]
        blue = match["alliances"][0]["endgamePoints"]
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["penaltyPoints"]
        blue = match["alliances"][0]["penaltyPoints"]
        return red, blue
    
class FreightFrenzy2021Adapter:
    """
    2021 exposes:
      ... endGamePoints, penaltyPoints, totalPoints, autonomousPoints, etc.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        if match["alliances"]:
            red = match["alliances"][1]["endgamePoints"]
            blue = match["alliances"][0]["endgamePoints"]
            return red, blue
        else:
            print(match)
            return None, None

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["penaltyPoints"]
        blue = match["alliances"][0]["penaltyPoints"]
        return red, blue
    
class Powerplay2022Adapater:
    """
    2022 exposes:
      ... endGamePoints, penaltyPoints, totalPoints, autonomousPoints, etc.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["endgamePoints"]
        blue = match["alliances"][0]["endgamePoints"]
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["penaltyPointsCommitted"]
        blue = match["alliances"][0]["penaltyPointsCommitted"]
        return red, blue

class Centerstage2023Adapter:
    """
    2023 exposes:
      ... endGamePoints, penaltyPoints, totalPoints, autonomousPoints, etc.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["endgamePoints"]
        blue = match["alliances"][0]["endgamePoints"]
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["penaltyPointsCommitted"]
        blue = match["alliances"][0]["penaltyPointsCommitted"]
        return red, blue

class IntoTheDeep2024Adapter:
    """
    Seasons exposing teleop park/ascent and 'foulPointsCommitted' per alliance.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["teleopParkPoints"] + match["alliances"][1]["teleopAscentPoints"]
        blue = match["alliances"][0]["teleopParkPoints"] + match["alliances"][0]["teleopAscentPoints"]
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["foulPointsCommitted"]
        blue = match["alliances"][0]["foulPointsCommitted"]
        return red, blue

class Decode2025Adapter:
    """
    2025 exposes:
      ... endGamePoints, penaltyPoints, totalPoints, autonomousPoints, etc.
    """
    def endgame_points(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["endgamePoints"]
        blue = match["alliances"][0]["endgamePoints"]
        return red, blue

    def penalties(self, match: Dict[str, Any]) -> Tuple[int, int]:
        red = match["alliances"][1]["foulPointsCommitted"]
        blue = match["alliances"][0]["foulPointsCommitted"]
        return red, blue

SCORE_ADAPTERS: Dict[int, ScoreAdapter] = {
    2019: Skystone2019Adapter(),
    2020: UltimateGoal2020Adapter(),
    2021: FreightFrenzy2021Adapter(),
    2022: Powerplay2022Adapater(),
    2023: Centerstage2023Adapter(),
    2024: IntoTheDeep2024Adapter(),
    2025: Decode2025Adapter(),
}
DEFAULT_SCORE_ADAPTER: ScoreAdapter = DefaultModernAdapter()
