import { createRoot } from 'react-dom/client';
import { App } from './App';
import { Overlay } from './Overlay';
import './style.css';
const overlay = new URLSearchParams(location.search).has('overlay');
document.documentElement.dataset.surface = overlay ? 'overlay' : 'settings';
createRoot(document.getElementById('root')!).render(overlay ? <Overlay/> : <App/>);
