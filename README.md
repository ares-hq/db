# 🤖 ARES-Database — Automated Ranking & Evaluation System

Welcome to **ARES-Database**, the backend engine for managing and updating team performance metrics for FTC competitions. This tool connects to the official FTC API, calculates OPR-based statistics, ranks teams, and stores everything in a Supabase database — all with intelligent update logic.

---

## 🚀 Setup Instructions

### 1. Clone the repository

```bash
git clone https://github.com/henrybon806/ARES-Database.git
cd ARES-Database
```

### 2. Create a `.env` file

Copy and paste this template into a new file called `.env`:

```env
SUPABASE_URL=https://your-project.supabase.co
SUPABASE_KEY=your-anon-or-service-role-key
FIRST_USERNAME=your-first-api-username
FIRST_PASS=your-first-api-password
```

### 3. Install dependencies

We recommend using a virtual environment:

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

### 4. Run the main script

```bash
python ManageDatabase.py
```

You can optionally run with debug:

```bash
python ManageDatabase.py --debug
```

Or edit `main()` in `ManageDatabase.py` to toggle `debug=True` and `force_update=True` as needed.

---

## 🧠 Features

- Fetches latest team data from the official FTC API
- Calculates Auto, TeleOp, Endgame, and Overall OPR
- Smart merging: only updates Supabase if data improves or when `force_update=True`
- Dynamically re-ranks teams after updates
- Easily extendable to other seasons or stat metrics

---

## 🛠 Helpful Notes

### Kill a Python process:
```bash
ps -ef | grep python3
kill {process_id}
```

### Run the app in the background:
```bash
chmod +x ./update_database.sh
nohup ./update_database.sh > monitor.log 2>&1 &
```

### View live logs:
```bash
tail -f monitor.log
```

---

## 📦 Project Structure

```
ARES-Database/
│
├── API_Library/           # API handlers, data models, math logic
├── .env                   # Environment variables (keep secret!)
├── ManageDatabase.py      # Main execution script
├── monitor_and_run.sh     # Script for auto-running and monitoring
├── requirements.txt       # Python dependencies
└── README.md              # You're here!
```

---

## 💡 Tips

- You can switch between `force_update=True` and `False` inside `fetch_and_save_to_database()` depending on whether you want to overwrite all data or only update improvements.
- Supabase conflicts are handled via `upsert()` using `teamNumber` as the key.
- For large batches, performance can be improved with connection pooling and async fetchers (planned for future releases).

---

## 👨‍💻 Author

**Henry Bonomolo**  
Email: hbono@berkeley.edu  
GitHub: [@henrybono](https://github.com/henrybono)

---

## 📜 License

MIT License. Feel free to use, improve, and share — just credit where credit is due!

---

_The ARES system — built to empower teams through stats, structure, and strategy._ ⚙️
