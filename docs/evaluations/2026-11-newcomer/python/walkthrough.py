"""Exercise fid's documented Python and CLI APIs against the throwaway database."""

import csv
import json
import re
import shutil
import sqlite3
import subprocess
import sys
from datetime import datetime
from pathlib import Path

try:
    import fid
except ImportError as error:
    fid = None
    FID_IMPORT_ERROR = error
else:
    FID_IMPORT_ERROR = None


def parse_time(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def require_fid():
    if fid is None:
        raise RuntimeError(f"import fid failed: {FID_IMPORT_ERROR}")


def fit_from_report(report):
    match = re.search(r"half-life\s+([^,\s]+),\s*baseline\s+([^,\s]+)", report)
    if match is None:
        raise ValueError("the calibration report has no half-life and baseline line")
    return match.group(1), match.group(2)


def stamp_facts(connection):
    require_fid()
    connection.execute("ALTER TABLE facts ADD COLUMN artifact_json TEXT")
    rows = connection.execute(
        """
        SELECT id, entity, field, value, source, measured_at
        FROM facts
        ORDER BY id
        """
    ).fetchall()
    errors = []
    for row in rows:
        try:
            content = {
                "id": row["id"],
                "entity": row["entity"],
                "field": row["field"],
                "value": row["value"],
                "source": row["source"],
                "measured_at": row["measured_at"],
            }
            half_life = "5.8y" if row["field"] == "ceo" else "3m"
            artifact = fid.stamp(
                content,
                source=row["source"],
                source_class="database",
                half_life=half_life,
                now=parse_time(row["measured_at"]),
            )
            connection.execute(
                "UPDATE facts SET artifact_json = ? WHERE id = ?",
                (json.dumps(artifact, sort_keys=True), row["id"]),
            )
        except Exception as error:
            errors.append(f"fact {row['id']}: {error}")
    connection.commit()
    if errors:
        raise RuntimeError("; ".join(errors))


def check_facts(connection, now):
    require_fid()
    rows = connection.execute(
        "SELECT id, artifact_json FROM facts ORDER BY id"
    ).fetchall()
    errors = []
    for row in rows:
        if row["artifact_json"] is None:
            errors.append(f"fact {row['id']}: no stamped artifact")
            continue
        try:
            artifact = json.loads(row["artifact_json"])
            assessment = fid.check(artifact, floor=0.7, now=now)
            expiry = fid.expires_at(artifact, floor=0.7)
            expiry_text = expiry.isoformat() if expiry is not None else "never"
            print(
                f"fact {row['id']}: status {assessment.status}; "
                f"expires_at {expiry_text}"
            )
        except Exception as error:
            errors.append(f"fact {row['id']}: {error}")
    if errors:
        raise RuntimeError("; ".join(errors))


def export_rechecks(connection, csv_path):
    rows = connection.execute(
        """
        SELECT entity, correct, observed_at, checked_at
        FROM rechecks
        ORDER BY entity, observed_at
        """
    ).fetchall()
    with csv_path.open("w", encoding="utf-8", newline="") as csv_file:
        writer = csv.writer(csv_file)
        writer.writerow(["entity", "correct", "observed_at", "checked_at"])
        writer.writerows(rows)
    print(f"exported {len(rows)} re-checks to {csv_path}")
    return rows


def calibrate_rechecks(connection, database_path, fits):
    csv_path = database_path.with_name("rechecks.csv")
    rows = export_rechecks(connection, csv_path)
    errors = []

    try:
        require_fid()
        calibration_rows = [
            (
                row["entity"],
                (
                    parse_time(row["checked_at"]) - parse_time(row["observed_at"])
                ).total_seconds(),
                bool(row["correct"]),
            )
            for row in rows
        ]
        calibration = fid.calibrate(calibration_rows, "synthetic_rechecks")
        registry_path = database_path.with_name("python-half-lives.yaml")
        registry_path.write_text(calibration.yaml, encoding="utf-8")
        fits["Python"] = fit_from_report(calibration.report)
        print(f"Python registry: {registry_path}")
    except Exception as error:
        errors.append(f"Python calibrate: {error}")

    executable = shutil.which("fid")
    if executable is None:
        print("CLI calibrate skipped: no fid command on PATH")
    else:
        try:
            result = subprocess.run(
                [
                    executable,
                    "calibrate",
                    str(csv_path),
                    "--type",
                    "synthetic_rechecks",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            if result.returncode != 0:
                detail = (result.stderr or result.stdout).strip()
                raise RuntimeError(
                    f"fid calibrate exited {result.returncode}: {detail}"
                )
            registry_path = database_path.with_name("cli-half-lives.yaml")
            registry_path.write_text(result.stdout, encoding="utf-8")
            fits["CLI"] = fit_from_report(result.stderr)
            print(f"CLI registry: {registry_path}")
        except Exception as error:
            errors.append(f"CLI calibrate: {error}")

    if errors:
        raise RuntimeError("; ".join(errors))


def print_fits(fits):
    if not fits:
        raise RuntimeError("no fit available; truth is half-life 30d, baseline 0.30")
    for method, (half_life, baseline) in fits.items():
        print(
            f"{method} fit: half-life {half_life}, baseline {baseline} | "
            "truth: half-life 30d, baseline 0.30"
        )


def run_step(number, function, *args):
    try:
        function(*args)
    except Exception as error:
        message = str(error).replace("\n", " | ")
        print(f"STEP {number} FAILED: {message}")
    else:
        print(f"STEP {number} ok")


def main():
    database_path = Path(sys.argv[1] if len(sys.argv) > 1 else "facts.db")
    connection = sqlite3.connect(database_path)
    connection.row_factory = sqlite3.Row
    try:
        now_text = connection.execute(
            "SELECT value FROM meta WHERE key = 'now'"
        ).fetchone()[0]
        now = parse_time(now_text)
        fits = {}
        run_step(1, stamp_facts, connection)
        run_step(2, check_facts, connection, now)
        run_step(3, calibrate_rechecks, connection, database_path, fits)
        run_step(4, print_fits, fits)
    finally:
        connection.close()


if __name__ == "__main__":
    main()
