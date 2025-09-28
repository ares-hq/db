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
    
    def pick_two_any(self, teams, color: str):
        """ Picks two teams from the given list of teams based on the specified color ('red' or 'blue')."""
        color = color.lower()
        out, seen = [], set()
        for t in teams or []:
            st = str(t.get('station','')).lower()
            if st.startswith(color):
                n = int(t.get('teamNumber', 0) or 0)
                if n not in seen:
                    out.append(n); seen.add(n)
                if len(out) == 2:
                    break
        while len(out) < 2:
            out.append(0)
        return out[0], out[1]
    
    def save_matches_for_event(self, event, eventData):
        if not eventData:
            return self.matches_data

        for match in eventData:
            teams = match.get('teams') or []
            r1, r2 = self.pick_two_any(teams, 'red')
            b1, b2 = self.pick_two_any(teams, 'blue')

            redAlliance = Alliance(
                color='red',
                team1=Team(teamNumber=r1),
                team2=Team(teamNumber=r2),
                combined_overallOPR=int(match.get('scoreRedFinal', 0)),
                date=match.get('actualStartTime', 'Unknown'),
                win=int(match.get('scoreRedFinal', 0)) > int(match.get('scoreBlueFinal', 0)),
                tele=int(match.get('scoreRedFinal', 0)) - int(match.get('scoreRedAuto', 0)) - int(match.get('scoreBlueFoul', 0)),
                penalty=int(match.get('scoreRedFoul', 0)),
                matchType=match.get('tournamentLevel', 'Unknown'),
                skip=True,
            )

            blueAlliance = Alliance(
                color='blue',
                team1=Team(teamNumber=b1),
                team2=Team(teamNumber=b2),
                combined_overallOPR=int(match.get('scoreBlueFinal', 0)),
                date=match.get('actualStartTime', 'Unknown'),
                win=int(match.get('scoreBlueFinal', 0)) > int(match.get('scoreRedFinal', 0)),
                tele=int(match.get('scoreBlueFinal', 0)) - int(match.get('scoreBlueAuto', 0)) - int(match.get('scoreRedFoul', 0)),
                penalty=int(match.get('scoreBlueFoul', 0)),
                matchType=match.get('tournamentLevel', 'Unknown'),
                skip=True,
            )
            
            self.matches_data[self.generate_hash(event, redAlliance)] = redAlliance
            self.matches_data[self.generate_hash(event, blueAlliance)] = blueAlliance
            
        return self.matches_data
