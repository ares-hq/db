import os
import logging
from API_Library import FirstAPI
from dotenv import load_dotenv
from supabase import create_client, Client
from API_Library.API_Models.Team import Team

class TeamDataProcessor:
    def __init__(self, supabase_url=None, supabase_key=None):
        if not supabase_url or not supabase_key:
            load_dotenv()
            supabase_url = os.getenv("SUPABASE_URL")
            supabase_key = os.getenv("SUPABASE_KEY")
            if not supabase_url or not supabase_key:
                raise RuntimeError("SUPABASE_URL and SUPABASE_KEY must be set in .env")
        
        self.supabase: Client = create_client(supabase_url, supabase_key)
        self.table = "season_2024"
        self.team_data = {}

    def fetch_season_data(self):
        first_api = FirstAPI()
        season = first_api.get_season()
        for team in season.teams.values():
            self.team_data[team.teamNumber] = team

    def merge_with_database(self, force_update=False):
        existing_data = self.supabase.table(self.table).select("*").execute().data or []

        for row in existing_data:
            team_number = row["teamNumber"]

            api_team = self.team_data[team_number] if team_number in self.team_data else Team(overallOPR=-100)

            if not force_update and api_team.overallOPR <= row.get("overallOPR", -1):
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
                )
                
                self.team_data[team_number] = db_team

    def update_rankings(self):
        teams = list(self.team_data.values())

        def assign_rank(key, rank_field=None, reverse=True):
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

    def fetch_and_save_to_database(self, debug=False):
        self.fetch_season_data()
        self.merge_with_database()
        self.update_rankings()

        serializable_data = []
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
                "profileUpdate": team_info.profileUpdate
            }
            serializable_data.append(team_dict)

        if not serializable_data:
            logging.warning("No teams data to upsert; exiting.")
            return

        resp = (
            self.supabase
            .table(self.table)
            .upsert(serializable_data, on_conflict="teamNumber")
            .execute()
        )

        count = len(resp.data or [])
        if debug:
            print(f"✅ Upserted {count} rows into `{self.table}`")

    def close(self):
        pass

def main(debug=True):
    if debug:
        logging.basicConfig(level=logging.INFO)
    processor = TeamDataProcessor()
    try:
        processor.fetch_and_save_to_database(debug=debug)
    finally:
        processor.close()
        logging.info("Done.")

if __name__ == "__main__":
    main()