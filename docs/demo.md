# Notification demonstration

Captured from the passing local HTTP fixture/PostgreSQL integration test. Run `scripts/demo.sh` to repeat it; test databases, owner/team UUIDs, and the injected detection time are isolated from production. The API’s normal synchronization and team/notification/image handlers run in this demonstration.

The fixture changes HP from 51 to 56 and replaces PNG bytes at an unchanged upstream URL. Both published image URLs remain readable, and the test compares their actual bytes. One event is returned across two current teams. A repeat synchronization creates no events; the inclusive seven-day boundary retains the alert, and one further second removes it. Removing the Pokémon from all current teams also hides the event.

```text
running 1 test
Baseline: {"notifications":0,"pokemon":{"active":true,"base_stat_total":306,"id":100,"name":"p1","species_id":1,"sprite_url":"/api/v1/sprites/4ff6ab670a58c14270e034e2090d9a432caa263a14e0a25785386b0c12f880b5","stats":{"attack":51,"defense":51,"hp":51,"special_attack":51,"special_defense":51,"speed":51},"types":["fire"]},"team_id":"daa203df-8cf7-41e2-9996-dd0ffb764347"}
After normal sync (same upstream sprite URL): {
  "limit": 60,
  "notifications": [
    {
      "affected_team_ids": [
        "da37f895-412c-41b3-8fda-ca3e77c7da37",
        "daa203df-8cf7-41e2-9996-dd0ffb764347"
      ],
      "descriptions": [
        "p1: sprite image content changed",
        "p1: hp changed from 51 to 56"
      ],
      "detected_at": "2027-01-15T09:00:00Z",
      "differences": {
        "sprite_hash": {
          "after": "6a34118ba2e0bf5da5ab14cb63b121e2e8b2987a876668a9b2f9c30e1357470b",
          "before": "4ff6ab670a58c14270e034e2090d9a432caa263a14e0a25785386b0c12f880b5"
        },
        "stats": {
          "after": {
            "attack": 51,
            "defense": 51,
            "hp": 56,
            "special_attack": 51,
            "special_defense": 51,
            "speed": 51
          },
          "before": {
            "attack": 51,
            "defense": 51,
            "hp": 51,
            "special_attack": 51,
            "special_defense": 51,
            "speed": 51
          }
        }
      },
      "id": 7,
      "pokemon": {
        "active": true,
        "base_stat_total": 311,
        "id": 100,
        "name": "p1",
        "species_id": 1,
        "sprite_url": "/api/v1/sprites/6a34118ba2e0bf5da5ab14cb63b121e2e8b2987a876668a9b2f9c30e1357470b",
        "stats": {
          "attack": 51,
          "defense": 51,
          "hp": 56,
          "special_attack": 51,
          "special_defense": 51,
          "speed": 51
        },
        "types": [
          "fire"
        ]
      }
    }
  ],
  "offset": 0,
  "total": 1
}
Repeated scan: 0 new changes; 1 deduplicated notification across 2 current teams.
Injected clock advanced seven days + one second: 0 notifications.
test change_notification_demo ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.45s
```
