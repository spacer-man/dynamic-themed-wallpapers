# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub's
"Report a vulnerability" button (Security → Advisories) on this repository.
Do not open a public issue for security problems.

You can expect an acknowledgement within a week.

## Scope

Things we especially care about:

- any data leaving the device other than the documented requests to Wikimedia / Wikidata / (opt-in) Unsplash;
- the Unsplash API key being readable by other apps or ending up in logs or backups;
- untrusted text from photo metadata (titles, descriptions, author names) being interpreted as markup or code
  anywhere in the UI.
