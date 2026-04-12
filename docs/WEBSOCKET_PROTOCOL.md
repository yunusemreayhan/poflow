# WebSocket Protocol — Estimation Rooms

## Endpoint

```
GET /api/rooms/{id}/ws?ticket=<ticket>
```

## Authentication

1. Obtain a ticket: `POST /api/timer/ticket` (requires Bearer token)
2. Connect to WebSocket with `?ticket=<ticket>` query parameter
3. Tickets expire after 30 seconds and are single-use
4. User must be a room member (verified on connection)

## Messages (Server → Client)

The server sends JSON-encoded `RoomState` objects whenever the room state changes:

```json
{
  "room": { "id": 1, "name": "Sprint Planning", "status": "voting", "current_task_id": 42, ... },
  "members": [{ "username": "alice", "role": "admin" }, ...],
  "current_task": { "id": 42, "title": "Implement feature X", ... },
  "votes": [{ "username": "alice", "voted": true, "value": null }, ...],
  "tasks": [...],
  "vote_history": [{ "task_id": 41, "task_title": "...", "average": 5.0, "consensus": true, "votes": [...] }]
}
```

- `votes[].value` is `null` until room status is `"revealed"`
- `vote_history` contains results from previously voted tasks

## Messages (Client → Server)

No client-to-server messages are supported. All actions (vote, reveal, etc.) go through the REST API.

## Room Status Flow

```
lobby → voting → revealed → lobby (or next task)
```

## Keep-Alive

Server sends WebSocket Ping frames every 30 seconds. Client should respond with Pong (handled automatically by most WebSocket libraries).

## Disconnection

Connection closes on:
- Client sends Close frame
- Server shuts down
- Ping timeout (no Pong response)
