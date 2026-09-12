title: Notes
author: Thom Bruce

# Research Notes

A second file in the same project as codex.ink. The names in double brackets
resolve *across files* to the entities defined there — open the whole folder and
the codex lists this file under "Referenced by" on [[Alice]], [[Bob]],
[[London]], and [[Paris]]. Clicking one of those backlinks opens this file.

Citations reference a source by its key: London's guild kept enrolment rolls
[@baker-guild-1988, p. 12], and the trade's history is surveyed elsewhere
[@ferber-2011]. Each `[@key]` resolves to a source entity below and earns it a
backlink, the way `[[wikilinks]]` do.

% Sources

%% Baker's Guild Records
id: baker-guild-1988
author: Worshipful Company of Bakers
year: 1988
title: Enrolment Rolls of the Bakers' Guild
place: London
publisher: Guild Press
kind: primary source

A primary-source entity, defined here rather than in codex.ink — the codex
gathers entities from every file in the project, and `[@baker-guild-1988]`
resolves to it. [[Alice]] trained here.

%% Ferber, E.
id: ferber-2011
author: Ferber, E.
year: 2011
title: A Social History of the London Bakehouse
place: London
publisher: Thameside

A secondary source. Enter the `author` surname-first (`Ferber, E.`) — that is how
the reference list prints it, and how the author-date short form finds the surname.
