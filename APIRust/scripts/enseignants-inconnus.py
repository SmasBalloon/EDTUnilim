#!/usr/bin/env python3
"""Les écritures d'enseignants qu'aucun nom ne réclame encore.

Parcourt les semaines publiées, relève les mentions d'enseignants et met de
côté celles que `data/teachers.csv` ne rattache à personne. Chacune est rendue
prête à coller dans le fichier, avec de quoi deviner qui elle désigne : le
nombre de créneaux, les ressources et les années concernées.

    python3 scripts/enseignants-inconnus.py [http://localhost:8080]

L'API doit tourner ; son adresse se passe en argument (`EDT_API` sinon).
"""

import json
import os
import sys
import urllib.error
import urllib.request
from collections import defaultdict


def read(base, path):
    with urllib.request.urlopen(f"{base}{path}", timeout=60) as response:
        return json.load(response)


def main():
    base = (sys.argv[1] if len(sys.argv) > 1 else os.environ.get("EDT_API", "http://localhost:8080")).rstrip("/")

    try:
        known = {teacher["name"] for teacher in read(base, "/api/v1/teachers")}
        years = read(base, "/api/v1/years")
    except (urllib.error.URLError, OSError) as error:
        sys.exit(f"API injoignable sur {base} : {error}")

    # Une mention est « inconnue » quand l'API la rend telle quelle, faute de
    # l'avoir rattachée : son nom n'est alors pas celui d'un enseignant déclaré.
    seen = defaultdict(lambda: {"count": 0, "resources": set(), "years": set()})
    for year in years:
        for week in year["weeks"]:
            try:
                timetable = read(base, f"/api/v1/years/{year['year']}/weeks/{week}")
            except urllib.error.HTTPError:
                continue
            for course in timetable["courses"]:
                name = course.get("teacher")
                if not name or name in known:
                    continue
                entry = seen[name]
                entry["count"] += 1
                entry["years"].add(year["year"])
                if course.get("resource"):
                    entry["resources"].add(course["resource"]["code"])

    if not seen:
        print("Toutes les écritures rencontrées portent un nom. Rien à compléter.")
        return

    print(f"{len(seen)} écriture(s) sans nom, à reporter dans data/teachers.csv :\n")
    for name, entry in sorted(seen.items(), key=lambda item: -item[1]["count"]):
        resources = ", ".join(sorted(entry["resources"])[:4]) or "—"
        years = "/".join(f"BUT {year}" for year in sorted(entry["years"]))
        creneaux = "créneau " if entry["count"] == 1 else "créneaux"
        print(f"# Nom ?;{name:<6} — {entry['count']:>3} {creneaux} : {resources} ({years})")


if __name__ == "__main__":
    main()
