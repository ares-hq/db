import os
from API_Library.APIClient import APIClient
from concurrent.futures import ThreadPoolExecutor, as_completed
from API_Library.APIParams import APIParams
from API_Library.API_Models.Team import Team
from API_Library.API_Models.Event import Event
from API_Library.API_Models.Season import Season
from API_Library.RobotMath import MatrixBuilder, MatrixMath as mm
from datetime import datetime, timedelta, timezone
from dateutil import parser
from datetime import datetime
from tqdm import tqdm

class FirstAPI:
    def __init__(self):
        self.client = APIClient("https://ftc-api.firstinspires.org/v2.0")

    def get_season_events(self, year: int):
        year = year or self.find_year()
        params = APIParams(path_segments=[year, 'events'])
        response = self.client.api_request(params)
        return [event.get("code") for event in response.get('events', [])]

    def get_future_season_events(self, year=None):
        year = year or self.find_year()
        today = datetime.now(timezone.utc).date()
        yesterday = today - timedelta(days=7)
        params = APIParams(path_segments=[year, 'events'])
        response = self.client.api_request(params)
        return [event.get("code") for event in response.get('events', []) if parser.isoparse(event.get("dateStart")).date() >= yesterday]

    def get_season(self, year=None, debug=False, events="Future"):
        year = year or self.find_year()
        if events == "All":
            events = self.get_season_events(year=year)
        else:
            events = self.get_future_season_events(year=year)
        season = Season(seasonCode=year)

        progress_bar = tqdm(total=len(events), desc="Processing Events", unit=" event") if debug else None

        max_threads = min(128, os.cpu_count() * 8)
        with ThreadPoolExecutor(max_threads) as executor:
            futures = [executor.submit(self.fetch_event_data_thread, event, year, season, progress_bar) for event in events]
            for future in as_completed(futures):
                future.result()

        if progress_bar:
            progress_bar.close()

        return season

    def fetch_event_data_thread(self, event, year, season, progress_bar):
        try:
            event_data = self.get_event_data(event, year)
            endgame_stats = self.get_endgame_stats(event, year)
            penalties = self.get_penalties(event, year)

            if event_data:
                modified_on_match_data = {}
                for match, endgame, penalty in zip(event_data, endgame_stats, penalties):
                    match.update({
                        "scoreRedEndgame": endgame["red"],
                        "scoreBlueEndgame": endgame["blue"],
                        "penaltyPointsRed": penalty["red"],
                        "penaltyPointsBlue": penalty["blue"]
                    })
                    for team in match['teams']:
                        team_number = team['teamNumber']
                        modified_on_match_data.setdefault(team_number, match.get('modifiedOn', 'Unknown'))

                matrix_builder = MatrixBuilder(event_data)
                team_opr_values = {
                    metric: mm.LSE(matrix_builder.binary_matrix, getattr(matrix_builder, f"{metric}_matrix"))
                    for metric in ["auto", "tele", "endgame", "penalties"]
                }

                event_obj = Event(eventCode=event)
                for team in matrix_builder.teams:
                    team_idx = matrix_builder.team_indices[team]
                    team_info = self.get_team_info(team, year)
                    team_info.teamNumber = team
                    team_info.autoOPR = team_opr_values["auto"][team_idx]
                    team_info.teleOPR = team_opr_values["tele"][team_idx]
                    team_info.endgameOPR = team_opr_values["endgame"][team_idx]
                    team_info.overallOPR = team_info.autoOPR + team_info.teleOPR
                    team_info.penalties = team_opr_values["penalties"][team_idx]
                    team_info.profileUpdate = modified_on_match_data.get(team, 'Unknown')
                    event_obj.teams.append(team_info)

                season.events[event] = event_obj
                for new_team in event_obj.teams:
                    existing = season.teams.get(new_team.teamNumber)
                    if not existing or new_team.overallOPR > existing.overallOPR:
                        season.teams[new_team.teamNumber] = new_team

        except Exception as e:
            print(f"Error processing event {event}: {e}")
        finally:
            if progress_bar:
                progress_bar.update(1)
                
    def get_event_data(self, eventCode, year=None):
        year = year or self.find_year()
        params = APIParams(path_segments=[year, 'matches', eventCode])
        response = self.client.api_request(params)
        return response.get('matches', [])

    def get_endgame_stats(self, eventCode, year=None):
        year = year or self.find_year()
        params = APIParams(path_segments=[year, 'scores', eventCode, 'qual'])
        response = self.client.api_request(params)
        result = []
        for match in response.get('matchScores', []):
            red = match["alliances"][1]["teleopParkPoints"] + match["alliances"][1]["teleopAscentPoints"]
            blue = match["alliances"][0]["teleopParkPoints"] + match["alliances"][0]["teleopAscentPoints"]
            result.append({"matchNumber": match["matchNumber"], "red": red, "blue": blue})
        return result

    def get_penalties(self, eventCode, year=None):
        year = year or self.find_year()
        params = APIParams(path_segments=[year, 'scores', eventCode, 'qual'])
        response = self.client.api_request(params)
        result = []
        for match in response.get('matchScores', []):
            red = match["alliances"][1]["foulPointsCommitted"]
            blue = match["alliances"][0]["foulPointsCommitted"]
            result.append({"matchNumber": match["matchNumber"], "red": red, "blue": blue})
        return result

    def get_team_info(self, team_number, year=None):
        year = year or self.find_year()
        team_info_params = APIParams(
            path_segments=[year, 'teams'],
            query_params={'teamNumber': str(team_number)}
        )
        team_info = self.client.api_request(team_info_params).get('teams')[0]
        team = Team(
            teamName=team_info.get('nameShort', "Unknown"),
            sponsors=str(team_info.get('nameFull', "Unknown")).replace("/", ", ").replace("&", ", ").rstrip(", "),
            location=f"{team_info.get('city', 'Unknown')}, {team_info.get('stateProv', 'Unknown')}, {team_info.get('country', 'Unknown')}"
        )
        return team

    @staticmethod
    def find_year():
        current_date = datetime.now()
        return current_date.year - 1 if current_date.month < 8 else current_date.year

def main():
    first_api = FirstAPI()
    season = first_api.get_season(year=2024, debug=True)
    print(f"Season Data: {season}")

if __name__ == "__main__":
    main()