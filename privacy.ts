import { replaySharingEnabled, setReplaySharing } from "./shared-replay.js";

const checkbox = document.querySelector<HTMLInputElement>("#share-replays")!;
const status = document.querySelector<HTMLElement>("#privacy-status")!;
checkbox.checked = replaySharingEnabled();
checkbox.addEventListener("change", () => {
  try {
    setReplaySharing(checkbox.checked);
    status.textContent = checkbox.checked ? "Replay sharing is on for newly opened lessons." : "Replay sharing is off.";
  } catch {
    checkbox.checked = false;
    status.textContent = "This browser cannot save this preference. Replay sharing stays off while storage is unavailable.";
  }
});
