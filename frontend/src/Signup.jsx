import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';

function Signup() {
    const [name, setName] = useState('');
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [error, setError] = useState('');
    const navigate = useNavigate();  // Add the navigate function

    const handleSubmit = async (event) => {
        event.preventDefault();
        setError('');  // Clear previous errors

        const response = await fetch('http://localhost:3111/api/auth/register', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ name, email, password }),
        });

        if (response.ok) {
            //const data = await response.json();
            console.log('Signup successful:', response);
            // Handle login success
            //localStorage.setItem('token', data.token); // Save the token
            navigate('/login'); // Redirect to the login page using navigate instead of window.location.href
            setError('Registered!');  // Clear any previous errors
          } else {
            console.error('Signup failed');
            const errMessage = await response.text();
            setError(errMessage || 'Invalid');  // Display error message from server or default to 'Invalid'
        }
    };

    return (
        <form onSubmit={handleSubmit}>
            <h2>Signup</h2>
            <label htmlFor="name">Name:</label>
            <input
                type="text"
                id="name"
                value={name}
                onChange={e => setName(e.target.value)}
                required
            />
            <label htmlFor="email">Email:</label>
            <input
                type="email"
                id="email"
                value={email}
                onChange={e => setEmail(e.target.value)}
                required
            />
            <label htmlFor="password">Password:</label>
            <input
                type="password"
                id="password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                required
            />
            <button type="submit">Signup</button>
            {error && <div style={{ color: 'red' }}>{error}</div>}
        </form>
    );
}

export default Signup;
