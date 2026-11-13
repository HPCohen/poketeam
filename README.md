# Poketeam
Pokemon team builder.

## Task 1: Team Builder
1. Display all Pokémon in a responsive grid showing name, type(s), stats, and sprite image.
2. Allow users to build a team of up to 6 Pokémon. Order matters; users can add, remove,
and reorder. Persist all team information so a user can access their teams across sessions.
3. Allow users to create, delete, and update multiple teams using the team builder UI.
4. Given a Pokémon team, create an optimal counter team of equal size. Open-ended. Show your creativity here!

## Task 2: Data Change Notifications
PokéAPI data can change on rare occasions.
1. Create a backend job to scan for changes in any data used by the team builder.
2. If a user's Pokémon were involved in a change within the last week, display a frontend alert to that user describing what changed.
3. Prove that this feature works in a demo.

## Task 3: Technical Document
1. Create a one-page README.md on key design decisions and assumptions, showing your technical acumen and communication skills.

# Architecture
## Backend API
- Get all pokemon
- Team CRUD
    - Add pokemon (6 max)
    - Remove pokemon
    - Change order
- Get counter team
    - Given team id, optimize based on max type advantage

## Frontend
- Pokemon viewer
- Team list
