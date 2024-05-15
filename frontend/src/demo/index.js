import demoDevices from './devices.json';

export function isDemoDevice(dongleId) {
  return demoDevices.some((d) => d.dongle_id === dongleId);
}

export function isDemoRoute(route) {
  return route === '164080f7933651c4|2024-03-03--06-46-42';
}

export function isDemo() {
  if (!window.location || !window.location.pathname) {
    return false;
  }
  return isDemoDevice(window.location.pathname.split('/')[1]);
}
