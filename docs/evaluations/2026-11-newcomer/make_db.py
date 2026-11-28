"""Create the deterministic SQLite input for the newcomer walkthroughs."""

import random
import sqlite3
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

NOW = datetime(2026, 11, 28, 12, 0, 0, tzinfo=timezone.utc)


def rfc3339(value):
    """Format an aware UTC datetime without losing sub-second re-check ages."""
    timespec = "microseconds" if value.microsecond else "seconds"
    return value.isoformat(timespec=timespec).replace("+00:00", "Z")


def create_database(path):
    rng = random.Random(7)
    connection = sqlite3.connect(path)
    try:
        with connection:
            connection.executescript(
                """
                CREATE TABLE meta (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE facts (
                    id INTEGER PRIMARY KEY,
                    entity TEXT,
                    field TEXT,
                    value TEXT,
                    source TEXT,
                    measured_at TEXT
                );
                CREATE TABLE rechecks (
                    entity TEXT,
                    observed_at TEXT,
                    checked_at TEXT,
                    correct INTEGER
                );
                """
            )
            connection.execute(
                "INSERT INTO meta (key, value) VALUES ('now', ?)",
                (rfc3339(NOW),),
            )

            facts = [
                (
                    1,
                    "company:aurora",
                    "ceo",
                    "Ada Lovelace",
                    "synthetic-company-register",
                    "2023-01-15T00:00:00Z",
                ),
                (
                    2,
                    "company:borealis",
                    "ceo",
                    "Grace Hopper",
                    "synthetic-company-register",
                    "2024-09-01T00:00:00Z",
                ),
                (
                    3,
                    "company:cygnus",
                    "ceo",
                    "Katherine Johnson",
                    "synthetic-company-register",
                    "2026-06-01T00:00:00Z",
                ),
                (
                    4,
                    "ship:atlas",
                    "position",
                    "51.5074N 0.1278W",
                    "synthetic-ais",
                    rfc3339(NOW - timedelta(minutes=3)),
                ),
                (
                    5,
                    "ship:beagle",
                    "position",
                    "1.3521N 103.8198E",
                    "synthetic-ais",
                    rfc3339(NOW - timedelta(days=2)),
                ),
            ]
            connection.executemany(
                """
                INSERT INTO facts
                    (id, entity, field, value, source, measured_at)
                VALUES (?, ?, ?, ?, ?, ?)
                """,
                facts,
            )

            rechecks = []
            for entity_number in range(200):
                entity = f"entity-{entity_number:03d}"
                for _ in range(10):
                    age_days = rng.uniform(0, 180)
                    observed_at = NOW - timedelta(days=age_days)
                    probability_correct = 0.3 + 0.7 * 0.5 ** (age_days / 30)
                    correct = int(rng.random() < probability_correct)
                    rechecks.append(
                        (
                            entity,
                            rfc3339(observed_at),
                            rfc3339(NOW),
                            correct,
                        )
                    )
            connection.executemany(
                """
                INSERT INTO rechecks (entity, observed_at, checked_at, correct)
                VALUES (?, ?, ?, ?)
                """,
                rechecks,
            )

            fact_count = connection.execute("SELECT count(*) FROM facts").fetchone()[0]
            recheck_count = connection.execute(
                "SELECT count(*) FROM rechecks"
            ).fetchone()[0]
            print(f"facts: {fact_count}")
            print(f"rechecks: {recheck_count}")
    finally:
        connection.close()


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: python make_db.py PATH")
    create_database(Path(sys.argv[1]))


if __name__ == "__main__":
    main()
