import hashlib
from typing import Dict
from API_Library.API_Models.Event import Alliance
from API_Library.API_Models.Team import Team

class MatchMaker:
    def __init__(self):
        self.matches_data: Dict[str, Alliance] = {}

    def generate_hash(self, event: str, alliance: Alliance) -> str:
        t1 = alliance.team1
        t2 = alliance.team2
        score = alliance.combined_overallOPR
        base_string = f"{event}-{alliance.color}-{t1}-{t2}-{score}"
        return hashlib.md5(base_string.encode()).hexdigest()

    def get_all_matches(self) -> Dict[str, Alliance]:
        return self.matches_data
    
    def save_matches_for_event(self, event, eventData):
        if not eventData:
            return self.matches_data
        for match in eventData:
            redAlliance = Alliance(
                color='red',
                team1=Team(teamNumber=match.get('teams')[0]['teamNumber']),
                team2=Team(teamNumber=match.get('teams')[1]['teamNumber']),
                combined_overallOPR=match.get('scoreRedFinal', 0),
                date=match.get('actualStartTime', 'Unknown'),
                skip=True,
            )
            blueAlliance = Alliance(
                color='blue',
                team1=Team(match.get('teams')[2]['teamNumber']),
                team2=Team(match.get('teams')[3]['teamNumber']),
                combined_overallOPR=match.get('scoreBlueFinal', 0),
                date=match.get('actualStartTime', 'Unknown'),
                skip=True,
            )
            
            self.matches_data[self.generate_hash(event, redAlliance)] = redAlliance
            self.matches_data[self.generate_hash(event, blueAlliance)] = blueAlliance
            
        return self.matches_data
