# Heart-rate pacing

In **heart-rate mode** the trainer sets the power for you, a little at a time, to keep your heart rate inside a band you choose. It suits long, steady rides — endurance and base miles, or an easy spin you'd otherwise ride too hard. As your heart rate creeps up over a long ride, the power comes down to match.

> **A training aid, not medical advice.** Heart-rate mode only steers the trainer: it can't tell whether your band is right for you, and a strap can misread. If you have a heart condition, or aren't sure what's safe for you, ask a doctor first. If you feel unwell, dizzy or faint, or have chest pain, stop riding.

## What you need

- A **heart-rate reading**: a chest strap, a watch that broadcasts heart rate, or a trainer that passes one on (see [[Connecting your trainer]]).
- A **trainer VeloPopuli can control**. Heart-rate mode sets the power in ERG, so a read-only trainer or a power meter on its own can't follow it.
- Your **max heart rate** and **FTP** under **Settings → Rider**. The band and the power limits are worked out from them unless you set your own.

## Starting it

On a ride without a workout, press **Heart rate** in the mode switch at the bottom of the ride screen, next to **Gradient** and **ERG**. The button is greyed out until a heart-rate reading arrives, and on a ride with a workout, where the workout's targets drive the trainer.

Once it's on:

- A **W target** tile appears next to your heart rate. It shows the power the trainer is holding and, underneath, what heart-rate mode is doing.
- The **heart-rate tile** turns blue below your band, green inside it and amber above it.
- The stepper in the controls shows your band, for example **120–139 bpm**.

Heart-rate mode is remembered for your next ride, as ERG is. If no heart rate arrives then, it holds a steady power and tells you so (see *When things go wrong*); press **Gradient** or **ERG** to leave it.

### The warm-up

In the first minute of a ride — or when your heart rate is more than 15 bpm below the band, or you weren't pedalling — heart-rate mode starts with a gentle warm-up: the power climbs slowly from half your FTP towards 70% of it, and the tile counts down (*Warming up 7:12*). The warm-up ends as soon as your heart rate is within 3 bpm of the bottom of the band, or when its time is up: 8 minutes unless you change it.

Switched on later in a ride, while you're pedalling and your heart rate isn't far below the band, it skips the warm-up and starts from the power you've been riding at.

## How it holds the band

Your heart answers a change of effort slowly: a minute or two passes before a harder or easier effort shows in full. So heart-rate mode doesn't chase every beat. Once a second it looks at where your heart rate is heading, and when it's about to leave the band it changes the power by a small step — 1–4% of your FTP, and bigger steps down when you're already above the band. Then it waits before changing again: 15 seconds after easing off, 30 seconds after raising. It aims up to 3 bpm inside each edge of the band, so small wobbles don't move the power at all.

On a long ride your heart rate tends to drift up at the same power. Heart-rate mode answers by bringing the power down.

The power never goes outside your limits (**Lowest power** and **Highest power** in Settings).

| The tile says | What's happening |
|---|---|
| *Starting…* | Heart-rate mode has just been switched on (for a second at most). |
| *Warming up 7:12* | The warm-up, with its time left. |
| *Holding* | Your heart rate is where it should be, so the power stays put. Also shown when it would like to change but can't: at a power limit, or just after a pause. |
| *Raising* | Your heart rate is below the band or heading under it: the power goes up, a step at a time. |
| *Easing off* | Your heart rate is above the band or heading over it: the power comes down, a step at a time. |
| *Cadence low – easing* | You seem to be struggling (see below). |
| *No heart rate* | No heart-rate reading for 30 seconds. |

## Changing the band

- **−** and **+** on the band stepper, the `↑`/`↓` keys, or the difficulty buttons on a [handlebar controller](Virtual-gears#controllers) move the whole band by 2 bpm, keeping its width. The top of the band goes no higher than your max heart rate, and the bottom no lower than 80 bpm.
- The first press saves the band under **Settings → Heart-rate mode**, so your next ride starts with it too. Clear the two band fields there to go back to the automatic band.
- Heart-rate mode follows the new band straight away, but in the same small steps, so give it a minute or two.

## Settings

**Settings → Heart-rate mode**. Leave a field blank (it shows *Automatic*) to have it worked out for you. Changes take effect during a ride within a second.

| Setting | Blank means | What it does |
|---|---|---|
| **Band, low** and **Band, high** | 65% and 75% of your max heart rate (120–139 bpm with the default of 185) | The band to hold. If the two are the wrong way round, or less than 4 bpm apart, VeloPopuli puts them in order; the top is never above your max heart rate. |
| **Lowest power** | 40% of your FTP | The trainer never goes below this. |
| **Highest power** | 105% of your FTP | The trainer never goes above this. |
| **Warm-up** | — (8 minutes to start with) | The longest warm-up, up to 30 minutes. It ends sooner once your heart rate is up; 0 skips it. |

## When things go wrong

**You're struggling.** If your cadence stays below 60 rpm for 5 seconds, or your power falls well short of the target (under three quarters of it for 10 seconds), heart-rate mode takes it that your legs are going: it cuts the power by 5% every 5 seconds and doesn't raise it, whatever your heart rate says. Once you're back above 70 rpm and at least 90% of the target, it waits 30 seconds and carries on as usual. The tile says *Cadence low – easing* even when it was the power that fell short.

**The heart-rate reading drops out.** For the first 30 seconds the power simply stays where it is. After that the tile says *No heart rate*, a banner says how much power it is holding, and the trainer holds that power as in ERG. Once readings have been steady for 10 seconds, pacing picks up again, raising the power no sooner than 30 seconds later. Readings that can't be right — below 30 bpm, more than 5 bpm above your max heart rate, or a jump of more than 20 bpm at once — are ignored.

**You pause.** Pausing the ride, or stopping pedalling for 3 seconds, freezes heart-rate mode: the power and the warm-up wait where they are. When you ride on, it waits 30 seconds before raising the power, while your heart rate settles.

**Warnings** appear in a banner over the road:

| Banner | What it means |
|---|---|
| *Heart rate is above your band even at your lowest power. Take it easy, or raise the band.* | For 3 minutes your heart rate has stayed above the band with the power already at **Lowest power**. |
| *Heart rate is below your band even at your highest power. Lower the band, or raise the power ceiling in Settings.* | For 3 minutes your heart rate has stayed below the band with the power already at **Highest power**. |
| *Heart rate close to your maximum – easing off.* | Your heart rate is within 3 bpm of your max. The power is cut by 10% every 5 seconds until it comes down. |
| *No heart rate from your strap – holding 150 W until it's back.* | No heart-rate reading for 30 seconds (with the power it is holding). |

## Leaving heart-rate mode

- **ERG** carries on at the power heart-rate mode had reached, to the nearest 10 W, so the resistance doesn't jump.
- **Gradient** hands the trainer back to the road.
- Coming back to **Heart rate** starts afresh: with a warm-up if your heart rate is well below the band or you've stopped pedalling, otherwise from the power you're riding at.

## Workouts, gears and the road

- Heart-rate mode is for rides without a workout. On a workout ride the workout's targets drive the trainer.
- As in ERG, gears and the road's gradient and surface don't change the resistance. Your speed on screen still comes from your power and the road.
- On a [handlebar controller](Virtual-gears#controllers), the difficulty buttons move the band and win over shifting, as in ERG.

## Trying it without hardware

The **demo rider** has a made-up heart that answers effort late and slowly and drifts up on long efforts, so you can watch heart-rate mode at work: start a ride with the demo rider and press **Heart rate**. The demo heart belongs to a rider with an FTP of 200 W, whatever yours is set to.

## Status

Heart-rate mode is new. It has been tested against simulated hearts and ridden with the demo rider, but **not yet with a real heart-rate strap and trainer**. If it misbehaves, use **Report a problem** in the app: the diagnostics report records what heart-rate mode decided each second. See [[Reporting problems]].
