# TheHand i3blocks Status Indicator

A clickable status indicator for i3blocks that shows TheHand's mute state.

## Installation

1. Copy the script to your i3blocks directory:
   ```bash
   cp thehand-status ~/.config/i3blocks/
   chmod +x ~/.config/i3blocks/thehand-status
   ```

2. Add this block to your `~/.config/i3blocks/config`:
   ```ini
   [thehand]
   command=~/.config/i3blocks/thehand-status
   interval=2
   markup=pango
   signal=10
   ```

3. Reload i3: `Mod+Shift+R`

## Usage

The indicator shows:
- **MIC✓** (green) - TheHand is active and listening
- **MIC✗** (red) - TheHand is muted
- **MIC-** (gray) - TheHand is not running

Click on the indicator to toggle mute on/off.

## Requirements

- i3blocks
- TheHand running with signal handler support (v0.1.0+)
