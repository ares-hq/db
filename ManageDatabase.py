import os
import logging
from API_Library import FirstAPI
from dotenv import load_dotenv
from supabase import create_client, Client
from API_Library.API_Models.Team import Team
from datetime import datetime
from zoneinfo import ZoneInfo

class TeamDataProcessor:
    def __init__(self, supabase_url=None, supabase_key=None):
        if not supabase_url or not supabase_key:
            load_dotenv(override=True)
            supabase_url = os.getenv("SUPABASE_URL")
            supabase_key = os.getenv("SUPABASE_KEY")
            if not supabase_url or not supabase_key:
                raise RuntimeError("SUPABASE_URL and SUPABASE_KEY must be set in .env")
        
        self.supabase: Client = create_client(supabase_url, supabase_key)
        self.table = "season_2024"
        self.match_table = "matches_2024"
        self.team_data = {}
        self.alliance_data = []
        self.first_api = FirstAPI()

    def fetch_season_data(self, debug=False, events='Future'):
        season = self.first_api.get_season(debug=debug, events=events)
        for team in season.teams.values():
            self.team_data[team.teamNumber] = team
        self.alliance_data = self.convert_alliances_to_serializable_format(season.matches)
        
    def convert_alliances_to_serializable_format(self, matches_dict):
        serializable = []
        for dashcode, alliance in matches_dict.items():
            serializable.append({
                "matchcode": dashcode,
                "team_1": int(alliance.team1.teamNumber),
                "team_2": int(alliance.team2.teamNumber),
                "totalPoints": int(alliance.combined_overallOPR),
                "alliance": alliance.color,
                "date": alliance.date,
                "matchType": alliance.matchType,
            })
        return serializable

    def merge_with_database(self, force_update=True):
        existing_data = self.supabase.table(self.table).select(
            "teamNumber,teamName,sponsors,location,autoOPR,teleOPR,endgameOPR,overallOPR,autoRank,teleRank,endgameRank,overallRank,penalties,penaltyRank,profileUpdate,eventDate"
        ).execute().data or []

        for row in existing_data:
            team_number = row["teamNumber"]

            api_team = self.team_data[team_number] if team_number in self.team_data else Team(overallOPR=-100)
            existing_events = set(row.get("eventsAttended") or [])
            new_api_events = api_team.eventsAttended if isinstance(api_team.eventsAttended, list) else []
            merged_events = sorted(existing_events.union(new_api_events))
            api_team.eventsAttended = merged_events
            self.team_data[team_number] = api_team

            if not force_update and api_team.overallOPR <= row.get("overallOPR", -1):
                row["eventsAttended"] = merged_events
                db_team = Team(
                    teamName=row["teamName"],
                    sponsors=row["sponsors"],
                    location=row["location"],
                    teamNumber = team_number,
                    autoOPR = row["autoOPR"],
                    teleOPR = row["teleOPR"],
                    endgameOPR = row["endgameOPR"],
                    overallOPR = row["overallOPR"],
                    penalties = row["penalties"],
                    autoRank = row.get("autoRank"),
                    teleRank = row.get("teleRank"),
                    endgameRank = row.get("endgameRank"),
                    overallRank = row.get("overallRank"),
                    penaltyRank = row.get("penaltyRank"),
                    profileUpdate = row.get("profileUpdate"),
                    eventDate = row.get("eventDate"),
                    teamLogo = row.get("teamLogo"),
                    founded= row.get("founded", 0),
                    website= row.get("website", ""),
                    eventsAttended=merged_events,
                    averagePlace= row.get("averagePlace", 0.0),
                )
                
                self.team_data[team_number] = db_team

    def update_rankings(self):
        teams = list(self.team_data.values())

        def assign_rank(key, rank_field=None, reverse=False):
            sorted_teams = sorted(teams, key=lambda t: getattr(t, key), reverse=reverse)
            current_rank = 1
            for i, team in enumerate(sorted_teams):
                if i > 0 and getattr(team, key) == getattr(sorted_teams[i - 1], key):
                    setattr(team, rank_field, getattr(sorted_teams[i - 1], rank_field))
                else:
                    setattr(team, rank_field, current_rank)
                current_rank += 1

        assign_rank("overallOPR", "overallRank")
        assign_rank("autoOPR", "autoRank")
        assign_rank("teleOPR", "teleRank")
        assign_rank("endgameOPR", "endgameRank")
        assign_rank("penalties", "penaltyRank", reverse=False)

    def fetch_and_save_to_database(self, debug=False, force_update=False, events='Future'):
        self.fetch_season_data(debug=debug, events=events)
        self.merge_with_database(force_update=force_update)
        self.update_rankings()
        self.first_api.set_team_logos(list(self.team_data.values()))

        serializable_data = []
        now = datetime.now(ZoneInfo("America/Los_Angeles"))
        formattedTime = now.strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3]
        for team_number, team_info in self.team_data.items():
            team_dict = {
                "teamNumber": int(team_number),
                "teamName": team_info.teamName,
                "sponsors": team_info.sponsors,
                "location": team_info.location,
                "autoOPR": float(team_info.autoOPR),
                "teleOPR": float(team_info.teleOPR),
                "endgameOPR": float(team_info.endgameOPR),
                "overallOPR": float(team_info.overallOPR),
                "autoRank": int(team_info.autoRank) if team_info.autoRank is not None else None,
                "teleRank": int(team_info.teleRank) if team_info.teleRank is not None else None,
                "endgameRank": int(team_info.endgameRank) if team_info.endgameRank is not None else None,
                "overallRank": int(team_info.overallRank) if team_info.overallRank is not None else None,
                "penalties": float(team_info.penalties),
                "penaltyRank": int(team_info.penaltyRank) if team_info.penaltyRank is not None else None,
                "profileUpdate": formattedTime,
                "eventDate": team_info.eventDate,
                "teamLogo": team_info.teamLogo,
                "founded": team_info.founded,
                "website": team_info.website,
                "eventsAttended": team_info.eventsAttended,
                "averagePlace": team_info.averagePlace,
            }
            serializable_data.append(team_dict)
            
        if serializable_data:
            resp = (
                self.supabase
                .table(self.table)
                .upsert(serializable_data, on_conflict="teamNumber")
                .execute()
            )
            
        if self.alliance_data:
            self.supabase.table(self.match_table).upsert(self.alliance_data, on_conflict="matchcode").execute()

        count = len(resp.data or [])
        if debug:
            print(f"✅ Upserted {len(self.alliance_data)} matches into `{self.match_table}`")
            print(f"✅ Upserted {count} rows into `{self.table}`")

    def close(self):
        pass

def main(debug=False):
    if debug:
        logging.basicConfig(level=logging.INFO)
    processor = TeamDataProcessor()
    try:
        if debug:
            processor.fetch_and_save_to_database(debug=debug, force_update=True, events='All')
        else: 
            processor.fetch_and_save_to_database(debug=debug)
    finally:
        processor.close()
        logging.info("Done.")

if __name__ == "__main__":
    main()