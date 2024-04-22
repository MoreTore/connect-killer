import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';

function Login() {
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [error, setError] = useState('');  // State to hold error messages
    const navigate = useNavigate();  // Add the navigate function

    const handleSubmit = async (event) => {
        event.preventDefault();
        setError('');  // Clear previous errors

        const response = await fetch('http://localhost:3111/api/auth/login', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ email, password }),
        });

        if (response.ok) {
            const data = await response.json();
            console.log('Login successful:', data);
            // Handle login success (e.g., redirect, save token)
        } else {
            console.error('Login failed');
            setError('Invalid email or password');  // Set error message
            // Optionally, handle different errors based on response status
            // if (response.status === 401) {
            //     setError('Unauthorized: Incorrect username or password');
            // } else if (response.status === 500) {
            //     setError('Server error: Please try again later');
            // }
        }
    };

    const handleSignupRedirect = () => {
        navigate('/register');  // Redirects to the Signup page
    };

    return (
        <form onSubmit={handleSubmit}>
            <h2>Login</h2>
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
            <button type="submit">Login</button>
            <button type="button" onClick={handleSignupRedirect}>Signup</button>
            {error && <div style={{ color: 'red' }}>{error}</div>}  {/* Display the error message */}
        </form>
    );
}

export default Login;