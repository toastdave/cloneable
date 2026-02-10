# Workflow Step Recorder (Tauri)

Desktop app that records global click and keyboard events, captures three screenshots per event, and saves everything to a local session folder for later review.

## Workflow
I started with planning until I ended up with a PRD.jsonc full of tasks. After that, I split the implementation into 2 isolated git worktrees: one for a more standard Cursor agent flow and one for a automated Ralph loop. 
Everything that follows is for the Cursor agent flow on the `main` branch.
The automated Ralph loop is in the `ralph` branch. All features are implemented in the `ralph` branch, whether it actually works or not.

## Setup

Prereqs:
- [Rust](https://rust-lang.org/tools/install/)
- [Bun](https://bun.com/docs/installation)
- [Tauri system dependencies](https://tauri.app/start/prerequisites/)

Install and run:
```bash
bun install
bun tauri dev
```

Build:
```bash
bun tauri build
```

## Usage

- Click **Start Recording**, interact with the desktop, then click **Stop Recording**.
- A session folder is written to the Tauri app data directory:
  - `recordings/<session_id>/recording.json`
  - `recordings/<session_id>/shots/*.png`
- On Linux, Wayland is not supported. Use an X11 session (e.g. GNOME on Xorg).

### Known bugs:
- Screenshots happen once recording is stopped, not actually tracjed with event capture.
- Crashes on keyboard input. This is specific to the `ralph` branhc, fixed in the `main` branch

## Completed vs. Descoped

Completed:
- Tauri desktop app with start/stop recording button.
- Rust backend global input listener (clicks + keypresses).
- Per-event capture of full screen, window crop (with fallback), and click crop.
- Local storage of JSON + screenshots in session folders.
- Minimal Step parsing

Descoped:
- Step annotation (titles, descriptions, action types).
- Auto-grouping of text input into a single action.
- Keyboard shortcuts

## Key Technical Decisions & Tradeoffs

First off here, my knowledge of Rust is basically nonexistent. So in all honesty, I wasn't making too many strong technical decisions. Normally I have plenty of opinions on the implementation, tech stack, etc. But here I was pretty dependent on the models to get my feet wet. I'll list a couple of things I had to decide on though:

- I went for minimal frontend for the sake of time and need. I can really get into a nice clean UI, so that one was sad to avoid
- No DB. I was actually kinda leaning the drizzle/sqlite route at first. I'm so database minded often and normally like getting everything configured cleanly in a repo before I get to the logic, but it just wasn't needed to start capturing events. GPT 5.2 gave me a nice gut check on this one. Claude would've had me configuring Drizzle for 20 min
- No Wayland support. I use Hyprland on Linux, and apparently security is real tight on event capture. It didn't seem worth the time to solve for this niche, even though it meant I had to setup my wife's mac to test this
- Hardcoded MacOS key character. Mac was crashing on keyboard input when it tried to map the code to the character, so I hardcoded for the sake of time to validate the capture. This only supports a US standard layout.

## What I’d Build Next

So I'd love to actually meet all of the requirements. But first thing I MUST do is fix that screenshot bug. They're completely useless at the moment. And after all of this, I'd love an extra requirement of a UI to view all of your recordings.

## AI Tooling

Tools used:
- Planning: ChatGPT vs Claude - pitted the two against each other to validate responses to the same prompts. Normally when I do this they start off a fair bit on different pages to slowly converging into a plan both agree on. Finally got ChatGPT output a PRD.jsonc that was used as reference for tasks for all agents.
- Implementation: 
  - On the `main` branch, I used a frontend and backend Cursor agents. Each tackled the tasks in their category as I validated the plans, code chagnes, and tested the implementation. 
  - On the `ralph` branch, I used my ralph shell scripts with the same prompt to knock tasks off the PRD one by one. This ran in the background completely in Opencode with ChatGPT 5.2 Codex. Agents did this almost 100% autonomously. I belive I tossed a build error and a crash log back in, but I thnk that's the extent I touched it.
- Skills: I've been testing out the skills more recently. There's a skills folder for specific tasks, each downlaoded from [Vercel Skills](https://skills.sh/). I'm still ironing out their impact.

Where AI helped:
- AI accelerated in basic all aspects of the project from planning to implementation and debugging. 

Where AI misled or required correction:
I think this is something I'm normally much more aware of in the ecosystems I know well. Here I found more when testing. A couple issues to note:
- The app crashes on Mac keyboard inputs. ChatGPT fixed this by patching the `rdev` crate, which was kinda insane to see, and then hardcoding the MacOS key character. I'm very hestitant to believe that popular crates can't adequately handle this. I'll have to dig into this more.
- The events are listed post stopping the session and then the screenshots are captured for each event, all of the same screen that's not actually the event. I actually need to understand the rust implementation to iron this out.
- In the planning phase, the models were getting pretty hung up on not capturing passwords. I had to nip that scope creep in the bud real quick. Security is important, but why do I need security if my app crashes on keyboard input?