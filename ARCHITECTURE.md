# Clawdis / Popabot Architecture

## Overview

Clawdis is a self-hosted personal AI assistant platform. At its core, it's a **TypeScript/Node.js application** that integrates the Pi agent SDK and connects to multiple messaging surfaces.

The "gateway" is just marketing speak for what is essentially:
- An Express HTTP server
- Pi SDK integration
- Messaging client connections
- Session management

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              YOUR DEVICE                                │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                      CLAWDIS GATEWAY                            │   │
│  │                   (TypeScript/Node.js App)                      │   │
│  │                                                                 │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │   │
│  │  │   Express   │  │   Session   │  │      Skill Loader       │ │   │
│  │  │   Server    │  │   Manager   │  │  (reads ~/popabot/*.md) │ │   │
│  │  │  :18789     │  │  (SQLite)   │  └─────────────────────────┘ │   │
│  │  └─────────────┘  └─────────────┘                              │   │
│  │                                                                 │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │                    PI AGENT SDK                         │   │   │
│  │  │  ┌─────────────────┐  ┌──────────────────────────────┐  │   │   │
│  │  │  │ pi-agent-core   │  │         pi-ai                │  │   │   │
│  │  │  │ (agent runtime) │  │ (multi-provider LLM access)  │  │   │   │
│  │  │  └─────────────────┘  └──────────────────────────────┘  │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  │                                                                 │   │
│  │  ┌─────────────────────────────────────────────────────────┐   │   │
│  │  │                 MESSAGING SURFACES                      │   │   │
│  │  │  ┌─────────┐ ┌──────────┐ ┌─────────┐ ┌─────────────┐  │   │   │
│  │  │  │WhatsApp │ │ Telegram │ │ Discord │ │  iMessage   │  │   │   │
│  │  │  │(Baileys)│ │ (grammY) │ │(discord │ │ (applescript│  │   │   │
│  │  │  │         │ │          │ │   .js)  │ │  + shortcuts│  │   │   │
│  │  │  └─────────┘ └──────────┘ └─────────┘ └─────────────┘  │   │   │
│  │  └─────────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌──────────────────┐                                                  │
│  │    Tailscale     │◄─── Secure remote access via private mesh VPN    │
│  │   (optional)     │                                                  │
│  └──────────────────┘                                                  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          LLM PROVIDERS                                  │
│  ┌───────────┐ ┌────────┐ ┌────────┐ ┌─────────┐ ┌────────────────┐   │
│  │ Anthropic │ │ OpenAI │ │ Google │ │ Groq    │ │ Local (Ollama) │   │
│  │ (Claude)  │ │ (GPT)  │ │(Gemini)│ │         │ │                │   │
│  └───────────┘ └────────┘ └────────┘ └─────────┘ └────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Gateway Server

The main entry point - an Express.js HTTP server.

| Component | Purpose |
|-----------|---------|
| `src/gateway/` | HTTP server, routing, auth |
| Port | 18789 (configurable) |
| Auth | Token-based (`~/.clawdis/clawdis.json`) |
| Bind | Loopback (localhost) or all interfaces |

### 2. Pi Agent SDK

The AI brain. NOT Claude Code - it's Mario Zechner's multi-model runtime.

| Package | Purpose |
|---------|---------|
| `@mariozechner/pi-agent-core` | Agent runtime, conversation loop, tool execution |
| `@mariozechner/pi-ai` | Multi-provider LLM abstraction layer |
| `@mariozechner/pi-utils` | Shared utilities |

**Supported LLM Providers:**
- Anthropic (Claude) - via API key or OAuth (Pro/Max subscription)
- OpenAI (GPT-4, etc.)
- Google (Gemini)
- Groq
- Mistral
- Cohere
- DeepSeek
- xAI (Grok)
- Cerebras
- OpenRouter
- Local models (Ollama, LM Studio)

### 3. Messaging Surfaces

How users interact with the assistant:

| Surface | Library | Protocol |
|---------|---------|----------|
| WhatsApp | Baileys | WhatsApp Web protocol (unofficial) |
| Telegram | grammY | Bot API (official) |
| Discord | discord.js | Bot API (official) |
| iMessage | AppleScript + Shortcuts | macOS automation |
| Signal | signal-cli | Signal protocol |
| WebChat | Built-in | HTTP/WebSocket |

### 4. Session Manager

Conversation state persistence using SQLite.

- Stores conversation history per surface/chat
- Manages context windows
- Handles multi-turn conversations

### 5. Skill System

Skills are markdown files that extend the agent's capabilities.

```
~/popabot/
├── IDENTITY.md      # Name, theme, emoji
├── SOUL.md          # Persona, personality, boundaries
├── USER.md          # User profile (learns over time)
├── AGENTS.md        # Operating instructions
├── TOOLS.md         # Tool usage notes
└── skills/          # Custom skills (SKILL.md files)
```

**Bundled Skills (42+):**

| Category | Skills |
|----------|--------|
| **Utilities** | calculator, unit-converter, currency-converter, qr-code, url-shortener |
| **Productivity** | calendar, reminders, notes, todo, timer, pomodoro |
| **Communication** | email-draft, sms-draft, translate |
| **Information** | weather, news, wikipedia, dictionary, wolfram-alpha |
| **Development** | github, gitlab, npm, pypi, docker, kubernetes |
| **Smart Home** | philips-hue, home-assistant, shortcuts |
| **Media** | spotify, youtube, podcast, image-gen |
| **Finance** | stocks, crypto, budget |
| **Health** | fitness, meditation, sleep |
| **Travel** | flights, hotels, maps, uber |
| **Shopping** | amazon, price-compare |
| **Social** | twitter, linkedin, reddit |
| **Files** | dropbox, google-drive, icloud |
| **Knowledge** | obsidian, notion, trello |

## Data Flow

```
User sends message (WhatsApp/Telegram/etc.)
              │
              ▼
┌─────────────────────────────┐
│   Messaging Surface Client  │  (Baileys/grammY/etc.)
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│      Gateway Router         │  Routes to correct session
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│     Session Manager         │  Loads conversation history
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│      Pi Agent Core          │  Processes with loaded skills
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│         Pi AI               │  Calls LLM provider (Claude/GPT/etc.)
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│     Tool Execution          │  If agent calls tools
└─────────────────────────────┘
              │
              ▼
┌─────────────────────────────┐
│   Response sent back        │  Via same messaging surface
└─────────────────────────────┘
```

## Tailscale Integration

**What is Tailscale/Tailnet?**

A private mesh VPN that lets you access the gateway remotely without exposing it to the public internet.

```
┌─────────────────┐         ┌─────────────────┐
│  Your Phone     │         │  Home Mac       │
│  (anywhere)     │◄───────►│  (running       │
│                 │ Tailnet │   Clawdis)      │
└─────────────────┘         └─────────────────┘
        │                           │
        └───── Encrypted ───────────┘
              Mesh VPN
```

**Modes:**
- `serve` - Expose gateway via Tailscale serve (HTTPS)
- `funnel` - Expose to public internet via Tailscale funnel
- `off` - Local only

**Benefits:**
- No port forwarding needed
- No public IP exposure
- End-to-end encrypted
- Works through NAT/firewalls

## Configuration

All config lives in `~/.clawdis/clawdis.json`:

```json
{
  "identity": {
    "name": "Popabot",
    "theme": "Stevie's AI sidekick",
    "emoji": "🤖"
  },
  "agent": {
    "workspace": "/Users/stevengonsalvez/popabot"
  },
  "gateway": {
    "port": 18789,
    "mode": "local",
    "auth": {
      "mode": "token",
      "token": "<generated-token>"
    },
    "bind": "loopback",
    "tailscale": {
      "mode": "serve",
      "resetOnExit": true
    }
  },
  "whatsapp": {},
  "telegram": {
    "enabled": false
  },
  "talk": {
    "modelId": "eleven_v3",
    "interruptOnSpeech": true
  }
}
```

## Voice Features (ElevenLabs)

When configured, enables:
- Text-to-speech responses
- Voice message transcription
- Conversational voice mode
- Interrupt on speech detection

## CLI Commands

```bash
# Start the gateway
pnpm clawdis start

# Run onboarding wizard
pnpm clawdis onboard

# Health check
pnpm clawdis health

# Link WhatsApp (QR code)
pnpm clawdis link whatsapp

# Link Telegram
pnpm clawdis link telegram

# View logs
pnpm clawdis logs

# Stop gateway
pnpm clawdis stop
```

## Chat Commands

When messaging Popabot, special commands start with `/`:

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/status` | Gateway and connection status |
| `/clear` | Clear conversation history |
| `/model <name>` | Switch LLM model |
| `/voice on/off` | Toggle voice responses |
| `/skill list` | List available skills |
| `/skill enable <name>` | Enable a skill |
| `/skill disable <name>` | Disable a skill |

## Security Model

1. **Local-first**: Runs on your machine, data stays local
2. **Token auth**: Gateway requires auth token for API access
3. **Loopback binding**: By default only accessible from localhost
4. **Tailscale**: Optional secure remote access (no public exposure)
5. **OAuth**: Claude Pro/Max uses OAuth (no API keys stored)

## Tech Stack Summary

| Layer | Technology |
|-------|------------|
| Runtime | Node.js 22+ |
| Language | TypeScript |
| Package Manager | pnpm |
| HTTP Server | Express.js |
| Database | SQLite |
| AI Runtime | Pi Agent SDK |
| WhatsApp | Baileys |
| Telegram | grammY |
| Voice | ElevenLabs |
| Remote Access | Tailscale |

## Project Structure

```
clawdis/
├── src/
│   ├── agents/           # Pi agent wrapper, tools
│   │   ├── bash-tools.ts # Shell execution tools
│   │   └── ...
│   ├── gateway/          # Express server, routing
│   ├── surfaces/         # Messaging clients
│   │   ├── whatsapp/     # Baileys integration
│   │   ├── telegram/     # grammY integration
│   │   └── ...
│   ├── skills/           # Skill loader + bundled skills
│   ├── voice/            # ElevenLabs integration
│   └── cli/              # CLI commands
├── packages/
│   └── ui/               # Web UI (React)
├── docs/                 # Documentation
└── package.json
```

## Current Popabot Status

| Component | Status |
|-----------|--------|
| Gateway | ✅ Running |
| Claude OAuth | ✅ Configured (Max subscription) |
| WhatsApp | ✅ Linked (+447563241014) |
| Telegram | ⏳ Not configured |
| ElevenLabs | ⏳ Not configured |
| Skills | ✅ All bundled skills enabled |

## Pending Setup

1. **Telegram**: Get bot token from @BotFather, run `pnpm clawdis link telegram`
2. **ElevenLabs**: Get API key, add to config for voice features
